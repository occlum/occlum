use crate::prelude::*;

bitflags::bitflags! {
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

bitflags::bitflags! {
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
