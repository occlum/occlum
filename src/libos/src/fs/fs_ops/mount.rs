use std::path::PathBuf;
use std::sync::Once;

use config::{parse_key, parse_mac, ConfigMount, ConfigMountFsType, ConfigMountOptions};
use rcore_fs_mountfs::MNode;
use util::mem_util::from_user;
use util::resolv_conf_util::write_resolv_conf;

use super::rootfs::{mount_nonroot_fs_according_to, open_root_fs_according_to, umount_nonroot_fs};
use super::*;

lazy_static! {
    static ref MOUNT_ONCE: Once = Once::new();
}

pub fn do_mount_rootfs(
    user_config: &config::Config,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<()> {
    debug!("mount rootfs");

    if MOUNT_ONCE.is_completed() {
        return_errno!(EPERM, "rootfs cannot be mounted more than once");
    }
    let new_rootfs = open_root_fs_according_to(&user_config.mount, user_key)?;
    mount_nonroot_fs_according_to(&new_rootfs.root_inode(), &user_config.mount, user_key, true)?;
    MOUNT_ONCE.call_once(|| {
        let mut rootfs = ROOT_FS.write().unwrap();
        rootfs.sync().expect("failed to sync old rootfs");
        *rootfs = new_rootfs;
        *ENTRY_POINTS.write().unwrap() = user_config.entry_points.to_owned();
    });
    // Write resolv.conf file into mounted file system
    write_resolv_conf()?;
    *RESOLV_CONF_STR.write().unwrap() = None;

    Ok(())
}

pub fn do_mount(
    source: &str,
    target: &str,
    flags: MountFlags,
    options: MountOptions,
) -> Result<()> {
    debug!(
        "mount: source: {}, target: {}, flags: {:?}, options: {:?}",
        source, target, flags, options
    );

    let target = if target == "/" {
        return_errno!(EPERM, "can not mount on root");
    } else if target.len() > 0 && target.as_bytes()[0] == b'/' {
        PathBuf::from(target)
    } else {
        let thread = current!();
        let fs = thread.fs().read().unwrap();
        PathBuf::from(fs.convert_to_abs_path(target))
    };

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

    let (mount_configs, user_key) = match options {
        MountOptions::UnionFS(unionfs_options) => {
            let mc = {
                let image_mc = ConfigMount {
                    type_: ConfigMountFsType::TYPE_SEFS,
                    target: target.clone(),
                    source: Some(unionfs_options.lower_dir.clone()),
                    options: Default::default(),
                };
                let container_mc = ConfigMount {
                    type_: ConfigMountFsType::TYPE_SEFS,
                    target: target.clone(),
                    source: Some(unionfs_options.upper_dir.clone()),
                    options: Default::default(),
                };

                ConfigMount {
                    type_: ConfigMountFsType::TYPE_UNIONFS,
                    target,
                    source: None,
                    options: ConfigMountOptions {
                        layers: Some(vec![image_mc, container_mc]),
                        ..Default::default()
                    },
                }
            };
            (vec![mc], unionfs_options.key)
        }
        MountOptions::SEFS(sefs_options) => {
            let mc = ConfigMount {
                type_: ConfigMountFsType::TYPE_SEFS,
                target,
                source: Some(sefs_options.dir.clone()),
                options: ConfigMountOptions {
                    mac: sefs_options.mac,
                    ..Default::default()
                },
            };
            (vec![mc], sefs_options.key)
        }
        MountOptions::HostFS(dir) => {
            let mc = ConfigMount {
                type_: ConfigMountFsType::TYPE_HOSTFS,
                target,
                source: Some(dir.clone()),
                options: Default::default(),
            };
            (vec![mc], None)
        }
        MountOptions::RamFS => {
            let mc = ConfigMount {
                type_: ConfigMountFsType::TYPE_RAMFS,
                target,
                source: None,
                options: Default::default(),
            };
            (vec![mc], None)
        }
    };

    let mut rootfs = ROOT_FS.write().unwrap();
    // Should we sync the fs before mount?
    rootfs.sync()?;
    let follow_symlink = !flags.contains(MountFlags::MS_NOSYMFOLLOW);
    mount_nonroot_fs_according_to(
        &rootfs.root_inode(),
        &mount_configs,
        &user_key,
        follow_symlink,
    )?;
    Ok(())
}

pub fn do_umount(target: &str, flags: UmountFlags) -> Result<()> {
    debug!("umount: target: {}, flags: {:?}", target, flags);

    let target = if target == "/" {
        return_errno!(EPERM, "cannot umount rootfs");
    } else if target.len() > 0 && target.as_bytes()[0] == b'/' {
        target.to_owned()
    } else {
        let thread = current!();
        let fs = thread.fs().read().unwrap();
        fs.convert_to_abs_path(target)
    };

    let mut rootfs = ROOT_FS.write().unwrap();
    // Should we sync the fs before umount?
    rootfs.sync()?;
    let follow_symlink = !flags.contains(UmountFlags::UMOUNT_NOFOLLOW);
    umount_nonroot_fs(&rootfs.root_inode(), &target, follow_symlink)?;
    Ok(())
}

bitflags! {
    pub struct MountFlags: u32 {
        const MS_RDONLY = 1;
        const MS_NOSUID = 2;
        const MS_NODEV = 4;
        const MS_NOEXEC = 8;
        const MS_SYNCHRONOUS = 16;
        const MS_REMOUNT = 32;
        const MS_MANDLOCK = 64;
        const MS_DIRSYNC = 128;
        const MS_NOSYMFOLLOW = 256;
        const MS_NOATIME = 1024;
        const MS_NODIRATIME = 2048;
        const MS_BIND = 4096;
        const MS_MOVE = 8192;
        const MS_REC = 16384;
        const MS_SILENT = 32768;
        const MS_POSIXACL = 1 << 16;
        const MS_UNBINDABLE = 1 << 17;
        const MS_PRIVATE = 1 << 18;
        const MS_SLAVE = 1 << 19;
        const MS_SHARED = 1 << 20;
        const MS_RELATIME = 1 << 21;
        const MS_KERNMOUNT = 1 << 22;
        const MS_I_VERSION = 1 << 23;
        const MS_STRICTATIME = 1 << 24;
        const MS_LAZYTIME = 1 << 25;
        const MS_SUBMOUNT = 1 << 26;
        const MS_NOREMOTELOCK = 1 << 27;
        const MS_NOSEC = 1 << 28;
        const MS_BORN = 1 << 29;
        const MS_ACTIVE = 1 << 30;
        const MS_NOUSER = 1 << 31;
    }
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

bitflags! {
    pub struct UmountFlags: u32 {
        const MNT_FORCE = 1;
        const MNT_DETACH = 2;
        const MNT_EXPIRE = 4;
        const UMOUNT_NOFOLLOW = 8;
    }
}

impl UmountFlags {
    pub fn from_u32(raw: u32) -> Result<Self> {
        let flags = Self::from_bits(raw).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
        if flags.contains(Self::MNT_EXPIRE)
            && (flags.contains(Self::MNT_FORCE) || flags.contains(Self::MNT_DETACH))
        {
            return_errno!(EINVAL, "MNT_EXPIRE with either MNT_DETACH or MNT_FORCE");
        }
        Ok(flags)
    }
}
