use super::hostfs::HostFS;
use super::sgx_impl::{SgxStorage, SgxUuidProvider};
use super::*;
use config::{ConfigMount, ConfigMountFsType};
use std::path::{Path, PathBuf};

use rcore_fs::vfs::{FileSystem, FileType, FsError, INode};
use rcore_fs_mountfs::{MNode, MountFS};
use rcore_fs_ramfs::RamFS;
use rcore_fs_sefs::dev::*;
use rcore_fs_sefs::SEFS;

lazy_static! {
    /// The root of file system
    pub static ref ROOT_INODE: Arc<INode> = {
        let mount_config = &config::LIBOS_CONFIG.mount;

        let rootfs = open_root_fs_according_to(mount_config)
            .expect("failed to create or open SEFS for /");
        let root = rootfs.root_inode();

        mount_nonroot_fs_according_to(mount_config, &root)
            .expect("failed to create or open other FS");

        root
    };
}

fn open_root_fs_according_to(mount_config: &Vec<ConfigMount>) -> Result<Arc<MountFS>, Error> {
    let (root_sefs_mac, root_sefs_source) = {
        let root_mount_config = mount_config
            .iter()
            .find(|m| m.target == Path::new("/"))
            .ok_or_else(|| Error::new(Errno::ENOENT, "The mount point at / is not specified"))?;

        if root_mount_config.type_ != ConfigMountFsType::TYPE_SEFS {
            return errno!(EINVAL, "The mount point at / must be SEFS");
        }
        if !root_mount_config.options.integrity_only {
            return errno!(EINVAL, "The root SEFS at / must be integrity-only");
        }
        if root_mount_config.source.is_none() {
            return errno!(
                EINVAL,
                "The root SEFS must be given a source path (on host)"
            );
        }
        (
            root_mount_config.options.mac,
            root_mount_config.source.as_ref().unwrap(),
        )
    };

    let root_sefs = SEFS::open(
        Box::new(SgxStorage::new(root_sefs_source, true, root_sefs_mac)),
        &time::OcclumTimeProvider,
        &SgxUuidProvider,
    )?;
    let root_mountable_sefs = MountFS::new(root_sefs);
    Ok(root_mountable_sefs)
}

fn mount_nonroot_fs_according_to(
    mount_config: &Vec<ConfigMount>,
    root: &MNode,
) -> Result<(), Error> {
    for mc in mount_config {
        if mc.target == Path::new("/") {
            continue;
        }

        if !mc.target.is_absolute() {
            return errno!(EINVAL, "The target path must be absolute");
        }
        if mc.target.parent().unwrap() != Path::new("/") {
            return errno!(EINVAL, "The target mount point must be under /");
        }
        let target_dirname = mc.target.file_name().unwrap().to_str().unwrap();

        use self::ConfigMountFsType::*;
        match mc.type_ {
            TYPE_SEFS => {
                if mc.options.integrity_only {
                    return errno!(EINVAL, "Cannot mount integrity-only SEFS at non-root path");
                }
                if mc.source.is_none() {
                    return errno!(EINVAL, "Source is expected for SEFS");
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
                    return errno!(EINVAL, "Source is expected for HostFS");
                }
                let source_path = mc.source.as_ref().unwrap();

                let hostfs = HostFS::new(source_path);
                mount_fs_at(hostfs, &root, target_dirname)?;
            }
            TYPE_RAMFS => {
                let ramfs = RamFS::new();
                mount_fs_at(ramfs, &root, target_dirname)?;
            }
        }
    }
    Ok(())
}

fn mount_fs_at(fs: Arc<dyn FileSystem>, parent_inode: &MNode, dirname: &str) -> Result<(), Error> {
    let mount_dir = match parent_inode.find(false, dirname) {
        Ok(existing_dir) => {
            if existing_dir.metadata()?.type_ != FileType::Dir {
                return errno!(EIO, "not a directory");
            }
            existing_dir
        }
        Err(_) => return errno!(ENOENT, "Mount point does not exist"),
    };
    mount_dir.mount(fs);
    Ok(())
}
