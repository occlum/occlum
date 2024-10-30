use super::dev_fs;
use super::hostfs::HostFS;
use super::procfs::ProcFS;
use super::sefs::{SgxStorage, SgxUuidProvider};
use super::*;
use crate::blk::{DevDisk, SwornDiskMeta, DEV_SWORNDISK};
use crate::config::{ConfigApp, ConfigMountFsType};
use crate::ctor::dtor;
use crate::fs::dev_fs::DevDiskOption;
use crate::time::OcclumTimeProvider;

use alloc::ffi::CString;
use alloc::sync::Weak;
use ext2_rs::{BlockDevice, Ext2};
use rcore_fs_mountfs::{MNode, MountFS};
use rcore_fs_ramfs::RamFS;
use rcore_fs_sefs::dev::*;
use rcore_fs_sefs::SEFS;
use rcore_fs_unionfs::UnionFS;
use std::path::{Path, PathBuf};
use std::untrusted::path::PathEx;

lazy_static! {
    /// The root of file system
    pub static ref ROOT_FS: RwLock<Arc<dyn FileSystem>> = {
        fn init_root_fs() -> Result<Arc<dyn FileSystem>> {
            let mount_config = &config::LIBOS_CONFIG.get_app_config("init").unwrap().mount;
            let rootfs = open_root_fs_according_to(mount_config, &None)?;
            mount_nonroot_fs_according_to(&rootfs.root_inode(), mount_config, &None, true)?;
            Ok(rootfs)
        }

        let rootfs = init_root_fs().unwrap_or_else(|e| {
            error!("failed to init root fs: {}", e.backtrace());
            panic!();
        });
        RwLock::new(rootfs)
    };
    /// All mounted SEFSes
    pub static ref SEFS_MANAGER: GlobalSefsManager = GlobalSefsManager::new();
}

pub fn open_root_fs_according_to(
    mount_configs: &Vec<ConfigMount>,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<Arc<dyn FileSystem>> {
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
        .ok_or_else(|| errno!(Errno::ENOENT, "the image SEFS in layers is not valid"))?;
    let root_image_sefs =
        open_or_create_sefs_according_to(&root_image_sefs_mount_config, user_key)?;
    // container SEFS in layers
    let root_container_sefs_mount_config = layer_mount_configs
        .iter()
        .find(|m| {
            m.target == Path::new("/")
                && m.type_ == ConfigMountFsType::TYPE_SEFS
                && m.options.mac.is_none()
                && m.options.index == 0
        })
        .ok_or_else(|| errno!(Errno::ENOENT, "the container SEFS in layers is not valid"))?;
    let root_container_sefs =
        open_or_create_sefs_according_to(&root_container_sefs_mount_config, user_key)?;
    // create UnionFS
    let root_unionfs = UnionFS::new(vec![root_container_sefs, root_image_sefs])?;
    let root_mountable_unionfs = MountFS::new(root_unionfs);
    Ok(root_mountable_unionfs)
}

pub fn umount_nonroot_fs(
    root: &Arc<dyn INode>,
    abs_path: &str,
    follow_symlink: bool,
) -> Result<()> {
    let mount_dir = if follow_symlink {
        root.lookup_follow(abs_path, MAX_SYMLINKS)?
    } else {
        let (dir_path, file_name) = split_path(abs_path);
        if file_name.ends_with("/") {
            root.lookup_follow(abs_path, MAX_SYMLINKS)?
        } else {
            root.lookup_follow(dir_path, MAX_SYMLINKS)?
                .lookup(file_name)?
        }
    };

    mount_dir.downcast_ref::<MNode>().unwrap().umount()?;
    Ok(())
}

pub fn mount_nonroot_fs_according_to(
    root: &Arc<dyn INode>,
    mount_configs: &Vec<ConfigMount>,
    user_key: &Option<sgx_key_128bit_t>,
    follow_symlink: bool,
) -> Result<()> {
    let mc_ext2_opt = mount_configs
        .iter()
        .find(|mc| mc.type_ == ConfigMountFsType::TYPE_EXT2);
    if let Some(mc) = mc_ext2_opt {
        // Setup disk metadata first if enabled Ext2
        setup_disk_meta_for_ext2(mc, user_key)?;
    }

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
                let sefs = open_or_create_sefs_according_to(&mc, user_key)?;
                mount_fs_at(sefs, root, &mc.target, follow_symlink)?;
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

                let hostfs = HostFS::new(source_path.unwrap());
                mount_fs_at(hostfs, root, &mc.target, follow_symlink)?;
            }
            TYPE_RAMFS => {
                let ramfs = RamFS::new();
                mount_fs_at(ramfs, root, &mc.target, follow_symlink)?;
            }
            TYPE_DEVFS => {
                let disk_options = if mc_ext2_opt.is_some() {
                    vec![DevDiskOption::new(DEV_SWORNDISK)]
                } else {
                    vec![]
                };
                let devfs = dev_fs::init_devfs(&disk_options)?;
                mount_fs_at(devfs, root, &mc.target, follow_symlink)?;
            }
            TYPE_PROCFS => {
                let procfs = ProcFS::new();
                mount_fs_at(procfs, root, &mc.target, follow_symlink)?;
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
                let unionfs = match (&image_fs_mc.type_, &container_fs_mc.type_) {
                    (TYPE_SEFS, TYPE_SEFS) => {
                        let image_sefs = open_or_create_sefs_according_to(image_fs_mc, user_key)?;
                        let container_sefs =
                            open_or_create_sefs_according_to(container_fs_mc, user_key)?;
                        UnionFS::new(vec![container_sefs, image_sefs])?
                    }
                    (_, _) => {
                        return_errno!(EINVAL, "Unsupported fs type inside unionfs");
                    }
                };
                mount_fs_at(unionfs, root, &mc.target, follow_symlink)?;
            }
            TYPE_EXT2 => {
                // Leave mounting the Ext2 to the final step
                // before the disk under the DevFS is ready
            }
        }
    }

    if let Some(mc) = mc_ext2_opt {
        let ext2 = open_ext2()?;
        mount_fs_at(ext2, root, &mc.target, follow_symlink)?;
    }
    Ok(())
}

pub fn mount_fs_at(
    fs: Arc<dyn FileSystem>,
    parent_inode: &Arc<dyn INode>,
    path: &Path,
    follow_symlink: bool,
) -> Result<()> {
    let path = path
        .to_str()
        .ok_or_else(|| errno!(EINVAL, "invalid path"))?;
    let mount_dir = if follow_symlink {
        parent_inode.lookup_follow(path, MAX_SYMLINKS)?
    } else {
        let (dir_path, file_name) = split_path(path);
        if file_name.ends_with("/") {
            parent_inode.lookup_follow(path, MAX_SYMLINKS)?
        } else {
            parent_inode
                .lookup_follow(dir_path, MAX_SYMLINKS)?
                .lookup(file_name)?
        }
    };
    mount_dir.downcast_ref::<MNode>().unwrap().mount(fs)?;
    Ok(())
}

fn open_or_create_sefs_according_to(
    mc: &ConfigMount,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<Arc<SEFS>> {
    assert!(mc.type_ == ConfigMountFsType::TYPE_SEFS);

    if mc.source.is_none() {
        return_errno!(EINVAL, "Source is expected for SEFS");
    }
    if mc.options.temporary && mc.options.mac.is_some() {
        return_errno!(EINVAL, "Integrity protected SEFS cannot be temporary");
    }
    let source_path = mc.source.as_ref().unwrap();
    let root_mac = mc.options.mac;
    let cache_size = mc.options.cache_size;
    let sefs = if !mc.options.temporary {
        if root_mac.is_some() {
            SEFS::open(
                Box::new(SgxStorage::new(
                    source_path,
                    user_key,
                    &root_mac,
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
                    &root_mac,
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
                    &root_mac,
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
                &root_mac,
                cache_size,
            )?),
            &time::OcclumTimeProvider,
            &SgxUuidProvider,
        )?
    };

    SEFS_MANAGER.add(&sefs);
    Ok(sefs)
}

fn open_ext2() -> Result<Arc<Ext2>> {
    debug_assert!(SwornDiskMeta::is_setup());

    let sworndisk = DevDisk::open_or_create(DEV_SWORNDISK)?.disk();
    let ext2 = match Ext2::open(sworndisk, Arc::new(OcclumTimeProvider)) {
        Err(e) if e == ext2_rs::FsError::WrongFs => {
            let sworndisk = format_disk_for_ext2()?;
            Ext2::open(sworndisk, Arc::new(OcclumTimeProvider))?
        }
        res => res?,
    };
    Ok(ext2)
}

fn format_disk_for_ext2() -> Result<Arc<dyn BlockDevice>> {
    // Format the SwornDisk using 'mke2fs' tool for Ext2
    let path = PathBuf::from("/sbin/mke2fs");
    let argv = vec![
        CString::new("mke2fs").unwrap(),
        CString::new("-q").unwrap(),
        CString::new("-t").unwrap(),
        CString::new("ext2").unwrap(),
        CString::new("/dev/".to_owned() + DEV_SWORNDISK).unwrap(),
    ];
    let pid = process::do_spawn(&path.to_str().unwrap(), &argv, &[], &[], None, &current!())?;
    let _ = process::do_wait4(pid as _, core::ptr::null_mut(), 0)?;

    let sworndisk = DevDisk::open_or_create(DEV_SWORNDISK)?.disk();
    Ok(sworndisk)
}

fn setup_disk_meta_for_ext2(mc: &ConfigMount, user_key: &Option<sgx_key_128bit_t>) -> Result<()> {
    debug_assert_eq!(mc.type_, ConfigMountFsType::TYPE_EXT2);
    let disk_size = mc.options.disk_size;
    if disk_size.is_none() {
        return_errno!(EINVAL, "Disk size is expected for Ext2");
    }
    let source_path = mc.source.as_ref();
    SwornDiskMeta::setup(disk_size.unwrap(), user_key, source_path)
}

/// Manage all mounted SEFSes globally.
pub struct GlobalSefsManager(RwLock<Vec<Weak<SEFS>>>);

impl GlobalSefsManager {
    pub fn new() -> Self {
        Self(RwLock::new(Vec::new()))
    }

    pub fn add(&self, sefs: &Arc<SEFS>) {
        self.0.write().unwrap().push(Arc::downgrade(sefs));
    }

    pub fn update(&self) {
        self.0
            .write()
            .unwrap()
            .retain(|sefs| sefs.upgrade().is_some());
    }

    pub fn sync_all(&self) -> Result<()> {
        for sefs in self
            .0
            .read()
            .unwrap()
            .iter()
            .filter_map(|sefs| sefs.upgrade())
        {
            sefs.sync()?;
        }
        Ok(())
    }

    pub fn update_then_sync_all(&self) -> Result<()> {
        let mut sefs_list = self.0.write().unwrap();
        sefs_list.retain(|sefs| sefs.upgrade().is_some());
        for sefs in sefs_list.iter().filter_map(|sefs| sefs.upgrade()) {
            sefs.sync()?;
        }
        Ok(())
    }

    pub fn clear(&self) {
        self.0.write().unwrap().clear();
    }
}

#[dtor]
fn sync_rootfs_when_exit() {
    crate::fs::ROOT_FS
        .read()
        .unwrap()
        .sync()
        .expect("sync rootfs when exit failed");
}
