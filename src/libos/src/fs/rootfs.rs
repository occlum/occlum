use super::hostfs::HostFS;
use super::sefs::{SgxStorage, SgxUuidProvider};
use super::*;
use config::{ConfigMount, ConfigMountFsType};
use std::path::{Path, PathBuf};

use rcore_fs_mountfs::{MNode, MountFS};
use rcore_fs_ramfs::RamFS;
use rcore_fs_sefs::dev::*;
use rcore_fs_sefs::SEFS;
use rcore_fs_unionfs::UnionFS;

lazy_static! {
    /// The root of file system
    pub static ref ROOT_INODE: Arc<dyn INode> = {
        fn init_root_inode() -> Result<Arc<dyn INode>> {
            let mount_config = &config::LIBOS_CONFIG.mount;
            let root_inode = {
                let rootfs = open_root_fs_according_to(mount_config)?;
                rootfs.root_inode()
            };
            mount_nonroot_fs_according_to(mount_config, &root_inode)?;
            Ok(root_inode)
        }

        init_root_inode().unwrap_or_else(|e| {
            error!("failed to init root inode: {}", e.backtrace());
            panic!();
        })
    };
}

fn open_root_fs_according_to(mount_configs: &Vec<ConfigMount>) -> Result<Arc<MountFS>> {
    let mount_config = mount_configs
        .iter()
        .find(|m| m.target == Path::new("/") && m.type_ == ConfigMountFsType::TYPE_UNIONFS)
        .ok_or_else(|| errno!(Errno::ENOENT, "the root UnionFS is not valid"))?;
    if mount_config.options.layers.is_none() {
        return_errno!(EINVAL, "The root UnionFS must be given the layers");
    }
    let layer_mount_configs = mount_config.options.layers.as_ref().unwrap();
    // image SEFS in layers
    let (root_image_sefs_mac, root_image_sefs_source) = {
        let mount_config = layer_mount_configs
            .iter()
            .find(|m| m.type_ == ConfigMountFsType::TYPE_SEFS && m.options.integrity_only)
            .ok_or_else(|| errno!(Errno::ENOENT, "the image SEFS in layers is not valid"))?;
        (
            mount_config.options.mac,
            mount_config.source.as_ref().unwrap(),
        )
    };
    let root_image_sefs = SEFS::open(
        Box::new(SgxStorage::new(
            root_image_sefs_source,
            true,
            root_image_sefs_mac,
        )),
        &time::OcclumTimeProvider,
        &SgxUuidProvider,
    )?;
    // container SEFS in layers
    let root_container_sefs_source = {
        let mount_config = layer_mount_configs
            .iter()
            .find(|m| m.type_ == ConfigMountFsType::TYPE_SEFS && !m.options.integrity_only)
            .ok_or_else(|| errno!(Errno::ENOENT, "the container SEFS in layers is not valid"))?;
        mount_config.source.as_ref().unwrap()
    };
    let root_container_sefs = {
        SEFS::open(
            Box::new(SgxStorage::new(root_container_sefs_source, false, None)),
            &time::OcclumTimeProvider,
            &SgxUuidProvider,
        )
    }
    .or_else(|_| {
        SEFS::create(
            Box::new(SgxStorage::new(root_container_sefs_source, false, None)),
            &time::OcclumTimeProvider,
            &SgxUuidProvider,
        )
    })?;

    let root_unionfs = UnionFS::new(vec![root_container_sefs, root_image_sefs])?;
    let root_mountable_unionfs = MountFS::new(root_unionfs);
    Ok(root_mountable_unionfs)
}

fn mount_nonroot_fs_according_to(mount_config: &Vec<ConfigMount>, root: &MNode) -> Result<()> {
    for mc in mount_config {
        if mc.target == Path::new("/") {
            continue;
        }

        if !mc.target.is_absolute() {
            return_errno!(EINVAL, "The target path must be absolute");
        }
        if mc.target.parent().unwrap() != Path::new("/") {
            return_errno!(EINVAL, "The target mount point must be under /");
        }
        let target_dirname = mc.target.file_name().unwrap().to_str().unwrap();

        use self::ConfigMountFsType::*;
        match mc.type_ {
            TYPE_SEFS => {
                if mc.options.integrity_only {
                    return_errno!(EINVAL, "Cannot mount integrity-only SEFS at non-root path");
                }
                if mc.source.is_none() {
                    return_errno!(EINVAL, "Source is expected for SEFS");
                }
                let source_path = mc.source.as_ref().unwrap();
                let sefs = {
                    SEFS::open(
                        Box::new(SgxStorage::new(source_path, false, None)),
                        &time::OcclumTimeProvider,
                        &SgxUuidProvider,
                    )
                }
                .or_else(|_| {
                    SEFS::create(
                        Box::new(SgxStorage::new(source_path, false, None)),
                        &time::OcclumTimeProvider,
                        &SgxUuidProvider,
                    )
                })?;
                mount_fs_at(sefs, &root, target_dirname)?;
            }
            TYPE_HOSTFS => {
                if mc.source.is_none() {
                    return_errno!(EINVAL, "Source is expected for HostFS");
                }
                let source_path = mc.source.as_ref().unwrap();

                let hostfs = HostFS::new(source_path);
                mount_fs_at(hostfs, &root, target_dirname)?;
            }
            TYPE_RAMFS => {
                let ramfs = RamFS::new();
                mount_fs_at(ramfs, &root, target_dirname)?;
            }
            TYPE_UNIONFS => {
                return_errno!(EINVAL, "Cannot mount UnionFS at non-root path");
            }
        }
    }
    Ok(())
}

fn mount_fs_at(fs: Arc<dyn FileSystem>, parent_inode: &MNode, dirname: &str) -> Result<()> {
    let mount_dir = match parent_inode.find(false, dirname) {
        Ok(existing_dir) => {
            if existing_dir.metadata()?.type_ != FileType::Dir {
                return_errno!(EIO, "not a directory");
            }
            existing_dir
        }
        Err(_) => return_errno!(ENOENT, "Mount point does not exist"),
    };
    mount_dir.mount(fs);
    Ok(())
}
