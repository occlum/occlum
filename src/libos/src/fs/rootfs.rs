use super::dev_fs;
use super::fs_view::{split_path, MAX_SYMLINKS};
use super::hostfs::HostFS;
use super::procfs::ProcFS;
use super::sefs::{SgxStorage, SgxUuidProvider};
use super::sync_fs_wrapper::{SyncFS, SyncInode};
use super::*;
use config::{ConfigApp, ConfigMountFsType};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::untrusted::path::PathEx;
use util::sgx::get_autokey_with_policy;

use async_mountfs::AsyncMountFS;
use async_sfs::AsyncSimpleFS;
use async_unionfs::AsyncUnionFS;
use block_device::{BlockDeviceAsFile, BLOCK_SIZE};
use jindisk::JinDisk;
use page_cache::{impl_fixed_size_page_alloc, CachedDisk};
use rcore_fs_ramfs::RamFS;
use rcore_fs_sefs::dev::*;
use rcore_fs_sefs::SEFS;
use sgx_disk::{HostDisk, IoUringDisk, SyncIoDisk};

/// Get or initilize the async rootfs
pub async fn rootfs() -> &'static Arc<dyn AsyncFileSystem> {
    loop {
        match STATE.compare_exchange(
            UNINITIALIZED,
            INITIALIZING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                let mount_config = &config::LIBOS_CONFIG.get_app_config("init").unwrap().mount;
                let rootfs = init_rootfs(mount_config, &None).await.unwrap_or_else(|e| {
                    error!("failed to init the rootfs: {}", e.backtrace());
                    panic!();
                });
                unsafe {
                    ROOT_FS = Some(rootfs);
                }
                STATE.store(INITIALIZED, Ordering::SeqCst);
                break;
            }
            Err(cur) if cur == INITIALIZED => break,
            Err(cur) => {
                // current state is INITIALIZING, try again
                assert!(cur == INITIALIZING);
            }
        }
    }
    // current state is INITIALIZED, return the rootfs
    unsafe { ROOT_FS.as_ref().unwrap() }
}

/// Update the rootfs, must be called after initialized
pub async fn update_rootfs(new_rootfs: Arc<dyn AsyncFileSystem>) -> Result<()> {
    loop {
        match STATE.compare_exchange(
            INITIALIZED,
            INITIALIZING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                // Disallow to update rootfs more than once
                if UPDATED.load(Ordering::SeqCst) == true {
                    STATE.store(INITIALIZED, Ordering::SeqCst);
                    return_errno!(EPERM, "rootfs cannot be updated more than once");
                }

                let old_rootfs = unsafe { ROOT_FS.as_ref().unwrap() };
                if let Err(e) = old_rootfs.sync().await {
                    STATE.store(INITIALIZED, Ordering::SeqCst);
                    return_errno!(e.errno(), "failed to sync old rootfs");
                }
                unsafe {
                    ROOT_FS = Some(new_rootfs);
                }
                UPDATED.store(true, Ordering::SeqCst);
                STATE.store(INITIALIZED, Ordering::SeqCst);
                break;
            }
            Err(cur) => {
                // current state must be INITIALIZING, try again
                assert!(cur == INITIALIZING);
            }
        }
    }
    // now the state is INITIALIZED
    Ok(())
}

/// Initialize the rootfs according to configurations
pub async fn init_rootfs(
    mount_configs: &Vec<ConfigMount>,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<Arc<dyn AsyncFileSystem>> {
    let rootfs = open_rootfs_according_to(mount_configs, user_key).await?;
    mount_nonroot_fs_according_to(&rootfs.root_inode().await, mount_configs, user_key, true)
        .await?;
    Ok(rootfs)
}

static mut ROOT_FS: Option<Arc<dyn AsyncFileSystem>> = None;
static STATE: AtomicUsize = AtomicUsize::new(UNINITIALIZED);
static UPDATED: AtomicBool = AtomicBool::new(false);
const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

async fn open_rootfs_according_to(
    mount_configs: &Vec<ConfigMount>,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<Arc<dyn AsyncFileSystem>> {
    let root_mount_config = mount_configs
        .iter()
        .find(|m| m.target == Path::new("/") && m.type_ == ConfigMountFsType::TYPE_UNIONFS)
        .ok_or_else(|| errno!(Errno::ENOENT, "the root UnionFS is not valid"))?;
    if root_mount_config.options.layers.is_none() {
        return_errno!(EINVAL, "the root UnionFS must be given the layers");
    }
    let layer_mount_configs = root_mount_config.options.layers.as_ref().unwrap();
    // image SEFS in layers
    let root_image_sefs_mount_config = layer_mount_configs
        .iter()
        .find(|m| {
            m.target == Path::new("/")
                && m.type_ == ConfigMountFsType::TYPE_SEFS
                && (m.options.mac.is_some() || m.options.index == 1)
        })
        .ok_or_else(|| errno!(Errno::ENOENT, "the image sefs in layers is not valid"))?;
    let root_image_sefs = SyncFS::new(open_or_create_sefs_according_to(
        &root_image_sefs_mount_config,
        user_key,
    )?);
    // TODO: Support AsyncSFS as root image layer

    // container AsyncSFS/SEFS in layers
    let root_container_fs_mount_config = layer_mount_configs
        .iter()
        .find(|m| {
            m.target == Path::new("/")
                && (m.type_ == ConfigMountFsType::TYPE_ASYNC_SFS
                    || m.type_ == ConfigMountFsType::TYPE_SEFS)
                && m.options.mac.is_none()
                && m.options.index == 0
        })
        .ok_or_else(|| errno!(Errno::ENOENT, "the container fs in layers is not valid"))?;
    let root_container_fs = match root_container_fs_mount_config.type_ {
        ConfigMountFsType::TYPE_ASYNC_SFS => {
            open_or_create_async_sfs_according_to(&root_container_fs_mount_config, user_key).await?
        }
        ConfigMountFsType::TYPE_SEFS => SyncFS::new(open_or_create_sefs_according_to(
            &root_container_fs_mount_config,
            user_key,
        )?),
        _ => unreachable!(),
    };

    // create UnionFS
    let root_unionfs = AsyncUnionFS::new(vec![root_container_fs, root_image_sefs]).await?;
    let root_mountable_unionfs = AsyncMountFS::new(root_unionfs);
    Ok(root_mountable_unionfs as _)
}

pub async fn mount_nonroot_fs_according_to(
    root: &Arc<dyn AsyncInode>,
    mount_configs: &Vec<ConfigMount>,
    user_key: &Option<sgx_key_128bit_t>,
    follow_symlink: bool,
) -> Result<()> {
    for mc in mount_configs {
        if mc.target == Path::new("/") {
            continue;
        }

        if !mc.target.is_absolute() {
            return_errno!(EINVAL, "The target path must be absolute");
        }

        use self::ConfigMountFsType::*;
        match mc.type_ {
            TYPE_SEFS => {
                let sefs = SyncFS::new(open_or_create_sefs_according_to(&mc, user_key)?);
                mount_nonroot_fs(sefs, root, &mc.target, follow_symlink).await?;
            }
            TYPE_HOSTFS => {
                let source_path =
                    mc.source.as_ref().and_then(
                        |source| {
                            if source.is_dir() {
                                Some(source)
                            } else {
                                None
                            }
                        },
                    );
                if source_path.is_none() {
                    return_errno!(EINVAL, "Source is expected for HostFS");
                }

                let hostfs = SyncFS::new(HostFS::new(source_path.unwrap()));
                mount_nonroot_fs(hostfs, root, &mc.target, follow_symlink).await?;
            }
            TYPE_RAMFS => {
                let ramfs = SyncFS::new(RamFS::new());
                mount_nonroot_fs(ramfs, root, &mc.target, follow_symlink).await?;
            }
            TYPE_DEVFS => {
                let devfs = SyncFS::new(dev_fs::init_devfs()?);
                mount_nonroot_fs(devfs, root, &mc.target, follow_symlink).await?;
                let ramfs = SyncFS::new(RamFS::new());
                mount_nonroot_fs(ramfs, root, &mc.target.join("shm"), follow_symlink).await?;
            }
            TYPE_PROCFS => {
                let procfs = ProcFS::new();
                mount_nonroot_fs(procfs, root, &mc.target, follow_symlink).await?;
            }
            TYPE_UNIONFS => {
                let layer_mcs = mc
                    .options
                    .layers
                    .as_ref()
                    .ok_or_else(|| errno!(EINVAL, "Invalid layers for unionfs"))?;
                let image_fs_mc = layer_mcs
                    .get(0)
                    .ok_or_else(|| errno!(EINVAL, "Invalid image layer"))?;
                let container_fs_mc = layer_mcs
                    .get(1)
                    .ok_or_else(|| errno!(EINVAL, "Invalid container layer"))?;
                let unionfs = {
                    let image_fs = match &image_fs_mc.type_ {
                        TYPE_SEFS => {
                            SyncFS::new(open_or_create_sefs_according_to(image_fs_mc, user_key)?)
                        }
                        TYPE_ASYNC_SFS => {
                            open_or_create_async_sfs_according_to(image_fs_mc, user_key).await?
                        }
                        _ => {
                            return_errno!(EINVAL, "Unsupported fs type inside unionfs");
                        }
                    };
                    let container_fs = match &container_fs_mc.type_ {
                        TYPE_SEFS => SyncFS::new(open_or_create_sefs_according_to(
                            container_fs_mc,
                            user_key,
                        )?),
                        TYPE_ASYNC_SFS => {
                            open_or_create_async_sfs_according_to(container_fs_mc, user_key).await?
                        }
                        _ => {
                            return_errno!(EINVAL, "Unsupported fs type inside unionfs");
                        }
                    };
                    AsyncUnionFS::new(vec![container_fs, image_fs]).await?
                };
                mount_nonroot_fs(unionfs, root, &mc.target, follow_symlink).await?;
            }
            TYPE_ASYNC_SFS => {
                let async_sfs = open_or_create_async_sfs_according_to(&mc, user_key).await?;
                mount_nonroot_fs(async_sfs, root, &mc.target, follow_symlink).await?;
            }
        }
    }
    Ok(())
}

pub async fn mount_nonroot_fs(
    fs: Arc<dyn AsyncFileSystem>,
    root: &Arc<dyn AsyncInode>,
    abs_path: &Path,
    follow_symlink: bool,
) -> Result<()> {
    let path = abs_path
        .to_str()
        .ok_or_else(|| errno!(EINVAL, "invalid path"))?;
    let mount_dir = if follow_symlink {
        root.lookup_follow(path, Some(MAX_SYMLINKS)).await?
    } else {
        if path.ends_with("/") {
            root.lookup_follow(path, Some(MAX_SYMLINKS)).await?
        } else {
            let (dir_path, file_name) = split_path(path);
            root.lookup_follow(dir_path, Some(MAX_SYMLINKS))
                .await?
                .lookup(file_name)
                .await?
        }
    };
    mount_dir.mount(fs).await?;
    Ok(())
}

pub async fn umount_nonroot_fs(
    root: &Arc<dyn AsyncInode>,
    abs_path: &str,
    follow_symlink: bool,
) -> Result<()> {
    let mount_dir = if follow_symlink {
        root.lookup_follow(abs_path, Some(MAX_SYMLINKS)).await?
    } else {
        if abs_path.ends_with("/") {
            root.lookup_follow(abs_path, Some(MAX_SYMLINKS)).await?
        } else {
            let (dir_path, file_name) = split_path(abs_path);
            root.lookup_follow(dir_path, Some(MAX_SYMLINKS))
                .await?
                .lookup(file_name)
                .await?
        }
    };

    mount_dir.umount().await?;
    Ok(())
}

async fn open_or_create_async_sfs_according_to(
    mc: &ConfigMount,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<Arc<dyn AsyncFileSystem>> {
    assert!(mc.type_ == ConfigMountFsType::TYPE_ASYNC_SFS);

    if mc.source.is_none() {
        return_errno!(EINVAL, "Source is expected for Async-SFS");
    }
    let source_path = {
        let mut source_path = mc.source.clone().unwrap();
        if source_path.is_dir() {
            source_path.push("async_sfs_image");
        }
        source_path
    };

    if mc.options.page_cache_size.is_none() {
        return_errno!(EINVAL, "Page cache size is expected for Async-SFS");
    }
    let page_cache_size = mc.options.page_cache_size.unwrap();
    // AsyncFsPageAlloc is a fixed-size allocator for page cache.
    impl_fixed_size_page_alloc! { AsyncFsPageAlloc, page_cache_size };

    let root_key = if let Some(user_key) = user_key {
        user_key.clone()
    } else {
        let autokey_policy = mc.options.autokey_policy;
        get_autokey_with_policy(&autokey_policy, &source_path)?
    };

    let async_sfs = if source_path.exists() {
        let cached_disk = {
            let sync_disk = SyncIoDisk::open(&source_path)?;
            let jindisk = JinDisk::open(Arc::new(sync_disk), root_key).await?;
            CachedDisk::<AsyncFsPageAlloc>::new(Arc::new(jindisk))?
        };
        AsyncSimpleFS::open(Arc::new(cached_disk)).await?
    } else {
        if mc.options.async_sfs_total_size.is_none() {
            return_errno!(EINVAL, "Total size is expected for Async-SFS");
        }
        let total_size = mc.options.async_sfs_total_size.unwrap();
        let cached_disk = {
            let total_blocks = total_size / BLOCK_SIZE;
            let sync_disk = SyncIoDisk::create_new(&source_path, total_blocks)?;
            let jindisk = JinDisk::create(Arc::new(sync_disk), root_key);
            CachedDisk::<AsyncFsPageAlloc>::new(Arc::new(jindisk))?
        };
        AsyncSimpleFS::create(Arc::new(cached_disk)).await?
    };
    Ok(async_sfs as _)
}

fn open_or_create_sefs_according_to(
    mc: &ConfigMount,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<Arc<SEFS>> {
    assert!(mc.type_ == ConfigMountFsType::TYPE_SEFS);

    if mc.source.is_none() {
        return_errno!(EINVAL, "Source is expected for SEFS");
    }
    let temporary = mc.options.temporary;
    let root_mac = mc.options.mac;
    let autokey_policy = mc.options.autokey_policy;
    if temporary && root_mac.is_some() {
        return_errno!(EINVAL, "Integrity protected SEFS cannot be temporary");
    }
    let source_path = mc.source.as_ref().unwrap();
    let cache_size = mc.options.sefs_cache_size;
    let sefs = if !temporary {
        if root_mac.is_some() {
            SEFS::open(
                Box::new(SgxStorage::new(
                    source_path,
                    user_key,
                    &root_mac,
                    &None,
                    cache_size,
                )?),
                &time::OcclumTimeProvider,
                &SgxUuidProvider,
            )?
        } else if source_path.join("metadata").exists() {
            SEFS::open(
                Box::new(SgxStorage::new(
                    source_path,
                    user_key,
                    &None,
                    &autokey_policy,
                    cache_size,
                )?),
                &time::OcclumTimeProvider,
                &SgxUuidProvider,
            )?
        } else {
            SEFS::create(
                Box::new(SgxStorage::new(
                    source_path,
                    user_key,
                    &None,
                    &autokey_policy,
                    cache_size,
                )?),
                &time::OcclumTimeProvider,
                &SgxUuidProvider,
            )?
        }
    } else {
        SEFS::create(
            Box::new(SgxStorage::new(
                source_path,
                user_key,
                &None,
                &autokey_policy,
                cache_size,
            )?),
            &time::OcclumTimeProvider,
            &SgxUuidProvider,
        )?
    };
    Ok(sefs)
}
