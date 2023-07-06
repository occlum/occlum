use crate::fs::{hostfs::HostFS, sync_fs_wrapper::SyncFS};
use config::{parse_key, parse_mac, ConfigMount, ConfigMountFsType, ConfigMountOptions};
use rcore_fs_ramfs::RamFS;
use rcore_fs_unionfs::UnionFS;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::Once;
use util::host_file_util::{write_host_file, HostFile};
use util::mem_util::from_user;

use super::rootfs::{init_rootfs, open_or_create_sefs_according_to, update_rootfs};
use super::*;

pub async fn do_mount_rootfs(
    user_app_config: &config::ConfigApp,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<()> {
    debug!("mount rootfs");

    let new_rootfs = init_rootfs(&user_app_config.mount, user_key).await?;

    // Update the rootfs
    update_rootfs(new_rootfs.clone()).await?;

    // Update the fs root of all remaining processes
    let new_root = Dentry::new_root(new_rootfs.root_inode().await);
    process::IDLE.fs().set_root(new_root.clone());
    let processes = crate::process::table::get_all_processes();
    for process in processes.iter() {
        if let Some(main_thread) = process.main_thread() {
            main_thread.fs().set_root(new_root.clone());
        }
    }

    // Update entry_points
    *ENTRY_POINTS.write().unwrap() = user_app_config.entry_points.to_owned();

    // Write resolv.conf file into mounted file system
    write_host_file(HostFile::ResolvConf).await?;
    *RESOLV_CONF_STR.write().unwrap() = None;

    // Write hostname file into mounted file system
    write_host_file(HostFile::HostName).await?;
    *HOSTNAME_STR.write().unwrap() = None;

    // Write hosts file into mounted file system
    write_host_file(HostFile::Hosts).await?;
    *HOSTS_STR.write().unwrap() = None;

    Ok(())
}

pub async fn do_mount(
    source: &str,
    target: &str,
    flags: MountFlags,
    options: MountOptions,
) -> Result<()> {
    debug!(
        "mount: source: {}, target: {}, flags: {:?}, options: {:?}",
        source, target, flags, options
    );

    let target = FsPath::try_from(target)?;
    if flags.contains(MountFlags::MS_REMOUNT)
        || flags.contains(MountFlags::MS_BIND)
        || flags.contains(MountFlags::MS_SHARED)
        || flags.contains(MountFlags::MS_PRIVATE)
        || flags.contains(MountFlags::MS_SLAVE)
        || flags.contains(MountFlags::MS_UNBINDABLE)
        || flags.contains(MountFlags::MS_MOVE)
    {
        return_errno!(EINVAL, "Only support to create a new mount");
    }

    let fs = match options {
        MountOptions::RamFS => SyncFS::new(RamFS::new()),
        MountOptions::HostFS(dir) => SyncFS::new(HostFS::new(dir)),
        MountOptions::SEFS(sefs_options) => {
            let mc = ConfigMount {
                type_: ConfigMountFsType::TYPE_SEFS,
                target: PathBuf::new(),
                source: Some(sefs_options.dir.clone()),
                options: ConfigMountOptions {
                    mac: sefs_options.mac,
                    ..Default::default()
                },
            };
            let sefs = open_or_create_sefs_according_to(&mc, &sefs_options.key)?;
            SyncFS::new(sefs)
        }
        MountOptions::UnionFS(unionfs_options) => {
            let image_mc = ConfigMount {
                type_: ConfigMountFsType::TYPE_SEFS,
                target: PathBuf::new(),
                source: Some(unionfs_options.lower_dir.clone()),
                options: Default::default(),
            };
            let container_mc = ConfigMount {
                type_: ConfigMountFsType::TYPE_SEFS,
                target: PathBuf::new(),
                source: Some(unionfs_options.upper_dir.clone()),
                options: Default::default(),
            };
            let image_sefs = open_or_create_sefs_according_to(&image_mc, &unionfs_options.key)?;
            let container_sefs =
                open_or_create_sefs_according_to(&container_mc, &unionfs_options.key)?;
            SyncFS::new(UnionFS::new(vec![container_sefs, image_sefs])?)
        }
    };

    let dir_dentry = {
        let current = current!();
        let fs = current.fs();
        if !flags.contains(MountFlags::MS_NOSYMFOLLOW) {
            fs.lookup(&target).await?
        } else {
            fs.lookup_no_follow(&target).await?
        }
    };
    dir_dentry.mount(fs).await?;
    Ok(())
}

pub async fn do_umount(target: &str, flags: UmountFlags) -> Result<()> {
    debug!("umount: target: {}, flags: {:?}", target, flags);

    let target = FsPath::try_from(target)?;

    let dir_dentry = {
        let current = current!();
        let fs = current.fs();
        if !flags.contains(UmountFlags::UMOUNT_NOFOLLOW) {
            fs.lookup(&target).await?
        } else {
            fs.lookup_no_follow(&target).await?
        }
    };
    dir_dentry.umount().await?;
    Ok(())
}

#[derive(Debug)]
pub enum MountOptions {
    UnionFS(UnionFSMountOptions),
    SEFS(SEFSMountOptions),
    HostFS(PathBuf),
    RamFS,
}

impl MountOptions {
    pub fn from_fs_type_and_options(type_: &ConfigMountFsType, options: *const i8) -> Result<Self> {
        Ok(match type_ {
            ConfigMountFsType::TYPE_SEFS => {
                let sefs_mount_options = {
                    let options = from_user::clone_cstring_safely(options)?
                        .to_string_lossy()
                        .into_owned();
                    SEFSMountOptions::from_input(options.as_str())?
                };
                Self::SEFS(sefs_mount_options)
            }
            ConfigMountFsType::TYPE_UNIONFS => {
                let unionfs_mount_options = {
                    let options = from_user::clone_cstring_safely(options)?
                        .to_string_lossy()
                        .into_owned();
                    UnionFSMountOptions::from_input(options.as_str())?
                };
                Self::UnionFS(unionfs_mount_options)
            }
            ConfigMountFsType::TYPE_HOSTFS => {
                let options = from_user::clone_cstring_safely(options)?
                    .to_string_lossy()
                    .into_owned();
                let dir = {
                    let options: Vec<&str> = options.split(",").collect();
                    let dir = options
                        .iter()
                        .find_map(|s| s.strip_prefix("dir="))
                        .ok_or_else(|| errno!(EINVAL, "no dir options"))?;
                    PathBuf::from(dir)
                };
                Self::HostFS(dir)
            }
            ConfigMountFsType::TYPE_RAMFS => Self::RamFS,
            _ => {
                return_errno!(EINVAL, "unsupported fs type");
            }
        })
    }
}

#[derive(Debug)]
pub struct UnionFSMountOptions {
    lower_dir: PathBuf,
    upper_dir: PathBuf,
    key: Option<sgx_key_128bit_t>,
}

impl UnionFSMountOptions {
    pub fn from_input(input: &str) -> Result<Self> {
        let options: Vec<&str> = input.split(",").collect();

        let lower_dir = options
            .iter()
            .find_map(|s| s.strip_prefix("lowerdir="))
            .ok_or_else(|| errno!(EINVAL, "no lowerdir options"))?;
        let upper_dir = options
            .iter()
            .find_map(|s| s.strip_prefix("upperdir="))
            .ok_or_else(|| errno!(EINVAL, "no upperdir options"))?;
        let key = match options.iter().find_map(|s| s.strip_prefix("key=")) {
            Some(key_str) => Some(parse_key(key_str)?),
            None => None,
        };

        Ok(Self {
            lower_dir: PathBuf::from(lower_dir),
            upper_dir: PathBuf::from(upper_dir),
            key,
        })
    }
}

#[derive(Debug)]
pub struct SEFSMountOptions {
    dir: PathBuf,
    key: Option<sgx_key_128bit_t>,
    mac: Option<sgx_aes_gcm_128bit_tag_t>,
}

impl SEFSMountOptions {
    pub fn from_input(input: &str) -> Result<Self> {
        let options: Vec<&str> = input.split(",").collect();

        let dir = options
            .iter()
            .find_map(|s| s.strip_prefix("dir="))
            .ok_or_else(|| errno!(EINVAL, "no dir options"))?;
        let key = match options.iter().find_map(|s| s.strip_prefix("key=")) {
            Some(key_str) => Some(parse_key(key_str)?),
            None => None,
        };
        let mac = match options.iter().find_map(|s| s.strip_prefix("mac=")) {
            Some(mac_str) => Some(parse_mac(mac_str)?),
            None => None,
        };

        Ok(Self {
            dir: PathBuf::from(dir),
            key,
            mac,
        })
    }
}
