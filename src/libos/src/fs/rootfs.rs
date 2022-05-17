use super::dev_fs;
use super::hostfs::HostFS;
use super::procfs::ProcFS;
use super::sefs::{SgxStorage, SgxUuidProvider};
use super::*;
use config::{ConfigApp, ConfigMountFsType};
use std::path::{Path, PathBuf};
use std::untrusted::path::PathEx;

use rcore_fs_mountfs::{MNode, MountFS};
use rcore_fs_ramfs::RamFS;
use rcore_fs_sefs::dev::*;
use rcore_fs_sefs::SEFS;
use rcore_fs_unionfs::UnionFS;

use util::mem_util::from_user;

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
                let devfs = dev_fs::init_devfs()?;
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
        }
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
    Ok(sefs)
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct user_rootfs_config {
    upper_layer_path: *const i8,
    lower_layer_path: *const i8,
    entry_point: *const i8,
    hostfs_source: *const i8,
    hostfs_target: *const i8,
}

impl user_rootfs_config {
    pub fn from_raw_ptr(ptr: *const user_rootfs_config) -> Result<user_rootfs_config> {
        let config = unsafe { *ptr };
        Ok(config)
    }
}

fn to_option_pathbuf(path: *const i8) -> Result<Option<PathBuf>> {
    let path = if path.is_null() {
        None
    } else {
        Some(PathBuf::from(
            from_user::clone_cstring_safely(path)?
                .to_string_lossy()
                .into_owned(),
        ))
    };

    Ok(path)
}

pub fn gen_config_app(config: &user_rootfs_config) -> Result<ConfigApp> {
    let upper_layer = to_option_pathbuf(config.upper_layer_path)?;
    let lower_layer = to_option_pathbuf(config.lower_layer_path)?;
    let entry_point = to_option_pathbuf(config.entry_point)?;
    let hostfs_source = to_option_pathbuf(config.hostfs_source)?;

    let hostfs_target = if config.hostfs_target.is_null() {
        PathBuf::from("/host")
    } else {
        PathBuf::from(
            from_user::clone_cstring_safely(config.hostfs_target)?
                .to_string_lossy()
                .into_owned(),
        )
    };

    let mut config_app = config::LIBOS_CONFIG.get_app_config("app").unwrap().clone();
    let root_mount_config = config_app
        .mount
        .iter_mut()
        .find(|m| m.target == Path::new("/") && m.type_ == ConfigMountFsType::TYPE_UNIONFS)
        .ok_or_else(|| errno!(Errno::ENOENT, "the root UnionFS is not valid"))?;

    if upper_layer.is_some() {
        let layer_mount_configs = root_mount_config.options.layers.as_mut().unwrap();
        // image SEFS in layers
        let root_image_sefs_mount_config = layer_mount_configs
            .iter_mut()
            .find(|m| {
                m.target == Path::new("/")
                    && m.type_ == ConfigMountFsType::TYPE_SEFS
                    && (m.options.mac.is_some() || m.options.index == 1)
            })
            .ok_or_else(|| errno!(Errno::ENOENT, "the image SEFS in layers is not valid"))?;

        root_image_sefs_mount_config.source = upper_layer;
        root_image_sefs_mount_config.options.mac = None;
        root_image_sefs_mount_config.options.index = 1;
    }

    if lower_layer.is_some() {
        let layer_mount_configs = root_mount_config.options.layers.as_mut().unwrap();
        // container SEFS in layers
        let root_container_sefs_mount_config = layer_mount_configs
            .iter_mut()
            .find(|m| {
                m.target == Path::new("/")
                    && m.type_ == ConfigMountFsType::TYPE_SEFS
                    && m.options.mac.is_none()
                    && m.options.index == 0
            })
            .ok_or_else(|| errno!(Errno::ENOENT, "the container SEFS in layers is not valid"))?;

        root_container_sefs_mount_config.source = lower_layer;
    }

    if entry_point.is_some() {
        config_app.entry_points.clear();
        config_app.entry_points.push(entry_point.unwrap())
    }

    if hostfs_source.is_some() {
        let hostfs_mount_config = config_app
            .mount
            .iter_mut()
            .find(|m| m.type_ == ConfigMountFsType::TYPE_HOSTFS)
            .ok_or_else(|| errno!(Errno::ENOENT, "the HostFS is not valid"))?;
        hostfs_mount_config.source = hostfs_source;
        hostfs_mount_config.target = hostfs_target;
    }

    Ok(config_app)
}
