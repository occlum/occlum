use crate::util::mem_util::from_user;

use super::time::{timespec_t, timeval_t, OcclumTimeProvider};
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

pub fn do_utimes(
    dirfd: i32,
    path: *const i8,
    atime: Option<Timespec>,
    mtime: Option<Timespec>,
    flags: i32,
) -> Result<()> {
    if path.is_null() && dirfd != AT_FDCWD {
        self::do_utimes_fd(dirfd as FileDesc, atime, mtime, flags)?;
    } else {
        let path = from_user::clone_cstring_safely(path)?
            .to_string_lossy()
            .into_owned();
        let flags =
            UtimeFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flag value"))?;
        let fs_path = FsPath::new(&path, dirfd, false)?;
        self::do_utimes_path(&fs_path, atime, mtime, flags)?;
    }
    Ok(())
}

fn do_utimes_fd(
    fd: FileDesc,
    atime: Option<Timespec>,
    mtime: Option<Timespec>,
    flags: i32,
) -> Result<()> {
    debug!(
        "utimes_fd: fd: {:?}, atime: {:?}, mtime: {:?}, flags: {:?}",
        fd, atime, mtime, flags
    );

    if flags != 0 {
        return_errno!(EINVAL, "invalid argument");
    }

    let file_ref = current!().file(fd)?;
    let mut info = file_ref.metadata()?;
    if let Some(atime) = atime {
        info.atime = atime;
    }
    if let Some(mtime) = mtime {
        info.mtime = mtime;
    }
    file_ref.set_metadata(&info)?;
    Ok(())
}

fn do_utimes_path(
    fs_path: &FsPath,
    atime: Option<Timespec>,
    mtime: Option<Timespec>,
    flags: UtimeFlags,
) -> Result<()> {
    debug!(
        "utimes_path: fs_path: {:?}, atime: {:?}, mtime: {:?}, flags: {:?}",
        fs_path, atime, mtime, flags
    );

    let inode = {
        let path = fs_path.to_abs_path()?;
        let current = current!();
        let fs = current.fs().read().unwrap();
        if flags.contains(UtimeFlags::AT_SYMLINK_NOFOLLOW) {
            fs.lookup_inode_no_follow(&path)?
        } else {
            fs.lookup_inode(&path)?
        }
    };
    let mut info = inode.metadata()?;
    if let Some(atime) = atime {
        info.atime = atime;
    }
    if let Some(mtime) = mtime {
        info.mtime = mtime;
    }
    inode.set_metadata(&info)?;
    Ok(())
}

pub fn get_utimes(
    times: Option<(timespec_t, timespec_t)>,
) -> Result<(Option<Timespec>, Option<Timespec>)> {
    let now = OcclumTimeProvider.current_time();
    if let Some(times) = times {
        let (atime, mtime) = times;

        if (!timespec_valid(atime)) || (!timespec_valid(mtime)) {
            return_errno!(EINVAL, "invalid argument: times")
        }

        let atime = if atime.nsec() == UTIME_OMIT {
            None
        } else if atime.nsec() == UTIME_NOW {
            Some(now)
        } else {
            Some(Timespec {
                sec: atime.sec(),
                nsec: atime.nsec(),
            })
        };
        let mtime = if mtime.nsec() == UTIME_OMIT {
            None
        } else if mtime.nsec() == UTIME_NOW {
            Some(now)
        } else {
            Some(Timespec {
                sec: mtime.sec(),
                nsec: mtime.nsec(),
            })
        };
        Ok((atime, mtime))
    } else {
        Ok((Some(now), Some(now)))
    }
}

fn timespec_valid(time: timespec_t) -> bool {
    if (time.nsec() == UTIME_NOW || time.nsec() == UTIME_OMIT) {
        true
    } else {
        time.sec() >= 0 && time.nsec() >= 0 && time.nsec() < 1_000_000_000
    }
}
