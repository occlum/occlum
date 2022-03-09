use super::locks::range_lock::c_flock;
use super::*;
use util::mem_util::from_user;

#[derive(Debug)]
pub enum FcntlCmd<'a> {
    /// Duplicate the file descriptor fd using the lowest-numbered available
    /// file descriptor greater than or equal to arg.
    DupFd(FileDesc),
    /// As for `DupFd`, but additionally set the close-on-exec flag for the
    /// duplicate file descriptor.
    DupFdCloexec(FileDesc),
    /// Return (as the function result) the file descriptor flags
    GetFd(),
    /// Set the file descriptor to be close-on-exec or not
    SetFd(u32),
    /// Get the file status flags
    GetFl(),
    /// Set the file status flags
    SetFl(u32),
    /// Test a file lock
    GetLk(&'a mut c_flock),
    /// Acquire or release a file lock
    SetLk(&'a c_flock),
    /// The blocking version of SetLK
    SetLkWait(&'a c_flock),
}

impl<'a> FcntlCmd<'a> {
    #[deny(unreachable_patterns)]
    pub fn from_raw(cmd: u32, arg: u64) -> Result<FcntlCmd<'a>> {
        Ok(match cmd as c_int {
            libc::F_DUPFD => FcntlCmd::DupFd(arg as FileDesc),
            libc::F_DUPFD_CLOEXEC => FcntlCmd::DupFdCloexec(arg as FileDesc),
            libc::F_GETFD => FcntlCmd::GetFd(),
            libc::F_SETFD => FcntlCmd::SetFd(arg as u32),
            libc::F_GETFL => FcntlCmd::GetFl(),
            libc::F_SETFL => FcntlCmd::SetFl(arg as u32),
            libc::F_GETLK => {
                let lock_mut_ptr = arg as *mut c_flock;
                from_user::check_mut_ptr(lock_mut_ptr)?;
                let lock_mut_c = unsafe { &mut *lock_mut_ptr };
                FcntlCmd::GetLk(lock_mut_c)
            }
            libc::F_SETLK => {
                let lock_ptr = arg as *const c_flock;
                from_user::check_ptr(lock_ptr)?;
                let lock_c = unsafe { &*lock_ptr };
                FcntlCmd::SetLk(lock_c)
            }
            libc::F_SETLKW => {
                let lock_ptr = arg as *const c_flock;
                from_user::check_ptr(lock_ptr)?;
                let lock_c = unsafe { &*lock_ptr };
                FcntlCmd::SetLkWait(lock_c)
            }
            _ => return_errno!(EINVAL, "unsupported command"),
        })
    }
}

pub fn do_fcntl(fd: FileDesc, cmd: &mut FcntlCmd) -> Result<isize> {
    debug!("fcntl: fd: {:?}, cmd: {:?}", &fd, cmd);

    let current = current!();
    let mut file_table = current.files().lock().unwrap();

    let ret = match cmd {
        FcntlCmd::DupFd(min_fd) => {
            let dup_fd = file_table.dup(fd, *min_fd, false)?;
            dup_fd as isize
        }
        FcntlCmd::DupFdCloexec(min_fd) => {
            let dup_fd = file_table.dup(fd, *min_fd, true)?;
            dup_fd as isize
        }
        FcntlCmd::GetFd() => {
            let entry = file_table.get_entry(fd)?;
            let fd_flags = if entry.is_close_on_spawn() {
                libc::FD_CLOEXEC
            } else {
                0
            };
            fd_flags as isize
        }
        FcntlCmd::SetFd(fd_flags) => {
            let entry = file_table.get_entry_mut(fd)?;
            entry.set_close_on_spawn((*fd_flags & libc::FD_CLOEXEC as u32) != 0);
            0
        }
        FcntlCmd::GetFl() => {
            let file = file_table.get(fd)?;
            let status_flags = file.status_flags()?;
            let access_mode = file.access_mode()?;
            (status_flags.bits() | access_mode as u32) as isize
        }
        FcntlCmd::SetFl(flags) => {
            let file = file_table.get(fd)?;
            let status_flags = StatusFlags::from_bits_truncate(*flags);
            file.set_status_flags(status_flags)?;
            0
        }
        FcntlCmd::GetLk(lock_mut_c) => {
            let file = file_table.get(fd)?;
            let lock_type = RangeLockType::from_u16(lock_mut_c.l_type)?;
            if RangeLockType::F_UNLCK == lock_type {
                return_errno!(EINVAL, "invalid flock type for getlk");
            }
            let mut lock = RangeLockBuilder::new()
                .type_(lock_type)
                .range(FileRange::from_c_flock_and_file(&lock_mut_c, &file)?)
                .build()?;
            file.test_advisory_lock(&mut lock)?;
            trace!("getlk returns: {:?}", lock);
            (*lock_mut_c).copy_from_range_lock(&lock);
            0
        }
        FcntlCmd::SetLk(lock_c) => {
            let file = file_table.get(fd)?;
            let lock = RangeLockBuilder::new()
                .type_(RangeLockType::from_u16(lock_c.l_type)?)
                .range(FileRange::from_c_flock_and_file(&lock_c, &file)?)
                .build()?;
            let is_nonblocking = true;
            file.set_advisory_lock(&lock, is_nonblocking)?;
            0
        }
        FcntlCmd::SetLkWait(lock_c) => {
            let file = file_table.get(fd)?;
            let lock = RangeLockBuilder::new()
                .type_(RangeLockType::from_u16(lock_c.l_type)?)
                .range(FileRange::from_c_flock_and_file(&lock_c, &file)?)
                .build()?;
            let is_nonblocking = false;
            file.set_advisory_lock(&lock, is_nonblocking)?;
            0
        }
    };
    Ok(ret)
}
