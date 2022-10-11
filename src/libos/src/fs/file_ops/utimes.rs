use crate::util::mem_util::from_user;

use super::time::{timespec_t, OcclumTimeProvider};
use super::*;

use rcore_fs::dev::TimeProvider;

const UTIME_NOW: i64 = (1i64 << 30) - 1i64;
pub const UTIME_OMIT: i64 = (1i64 << 30) - 2i64;

bitflags! {
    pub struct UtimeFlags: i32 {
        const AT_SYMLINK_NOFOLLOW = 1 << 8;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct utimbuf_t {
    atime: time_t,
    mtime: time_t,
}

impl utimbuf_t {
    pub fn atime(&self) -> time_t {
        self.atime
    }

    pub fn mtime(&self) -> time_t {
        self.mtime
    }
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum Utime {
    UTIME_OMIT,
    UTIME_NOW,
    UTIME(Timespec),
}

pub fn get_utimes(times: Option<(timespec_t, timespec_t)>) -> Result<(Utime, Utime)> {
    let now = OcclumTimeProvider.current_time();
    if let Some(times) = times {
        let (atime, mtime) = times;

        if (!timespec_valid(atime)) || (!timespec_valid(mtime)) {
            return_errno!(EINVAL, "parameter: times is invalid")
        }

        let atime = if atime.nsec() == UTIME_OMIT {
            Utime::UTIME_OMIT
        } else if atime.nsec() == UTIME_NOW {
            Utime::UTIME(now)
        } else {
            Utime::UTIME(Timespec {
                sec: atime.sec(),
                nsec: atime.nsec(),
            })
        };
        let mtime = if mtime.nsec() == UTIME_OMIT {
            Utime::UTIME_OMIT
        } else if mtime.nsec() == UTIME_NOW {
            Utime::UTIME(now)
        } else {
            Utime::UTIME(Timespec {
                sec: mtime.sec(),
                nsec: mtime.nsec(),
            })
        };
        Ok((atime, mtime))
    } else {
        Ok((Utime::UTIME(now), Utime::UTIME(now)))
    }
}

fn timespec_valid(time: timespec_t) -> bool {
    if (time.nsec() == UTIME_NOW || time.nsec() == UTIME_OMIT) {
        true
    } else {
        time.sec() >= 0 && time.nsec() >= 0 && time.nsec() < 1_000_000_000
    }
}

pub async fn do_utimes_fd(fd: FileDesc, atime: Utime, mtime: Utime, flags: i32) -> Result<()> {
    debug!(
        "utimes_fd: fd: {:?}, atime: {:?}, mtime: {:?}, flags: {:?}",
        fd, atime, mtime, flags
    );

    if flags != 0 {
        return_errno!(EINVAL, "parameter: flags is invalid");
    }

    let file_ref = current!().file(fd)?;

    if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        let inode = async_file_handle.dentry().inode();
        let mut info = inode.metadata().await?;
        if let Utime::UTIME(atime) = atime {
            info.atime = atime;
        }
        if let Utime::UTIME(mtime) = mtime {
            info.mtime = mtime;
        }
        inode.set_metadata(&info).await?;
    } else {
        return_errno!(EBADF, "not an inode");
    }
    Ok(())
}

pub async fn do_utimes_path(
    fs_path: &FsPath,
    atime: Utime,
    mtime: Utime,
    flags: UtimeFlags,
) -> Result<()> {
    debug!(
        "utimes_path: fs_path: {:?}, atime: {:?}, mtime: {:?}, flags: {:?}",
        fs_path, atime, mtime, flags
    );

    let inode = {
        let current = current!();
        let fs = current.fs();
        if flags.contains(UtimeFlags::AT_SYMLINK_NOFOLLOW) {
            fs.lookup_inode_no_follow(fs_path).await?
        } else {
            fs.lookup_inode(fs_path).await?
        }
    };
    let mut info = inode.metadata().await?;
    if let Utime::UTIME(atime) = atime {
        info.atime = atime;
    }
    if let Utime::UTIME(mtime) = mtime {
        info.mtime = mtime;
    }
    inode.set_metadata(&info).await?;
    Ok(())
}
