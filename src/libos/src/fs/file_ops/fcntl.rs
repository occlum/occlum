use super::*;
use async_io::file::flock_c;
use util::mem_util::from_user;

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
        FcntlCmd::SetFd(is_close_on_spawn) => {
            let entry = file_table.get_entry_mut(fd)?;
            entry.set_close_on_spawn(*is_close_on_spawn);
            0
        }
        FcntlCmd::GetFl() => {
            let file = file_table.get(fd)?;
            let status_flags = file.status_flags();
            let access_mode = file.access_mode();
            (status_flags.bits() | access_mode as u32) as isize
        }
        FcntlCmd::SetFl(status_flags) => {
            let file = file_table.get(fd)?;
            file.set_status_flags(*status_flags)?;
            0
        }
        FcntlCmd::GetLk(flock_mut_c) => {
            let file = file_table.get(fd)?;
            let inode_file = file
                .as_inode_file()
                .ok_or_else(|| errno!(EBADF, "not an inode file"))?;
            let mut lock = Flock::from_c(*flock_mut_c)?;
            if let FlockType::F_UNLCK = lock.l_type {
                return_errno!(EINVAL, "invalid flock type for getlk");
            }
            inode_file.test_advisory_lock(&mut lock)?;
            (*flock_mut_c).copy_from_safe(&lock);
            0
        }
        FcntlCmd::SetLk(flock_c) => {
            let file = file_table.get(fd)?;
            let inode_file = file
                .as_inode_file()
                .ok_or_else(|| errno!(EBADF, "not an inode file"))?;
            let lock = Flock::from_c(*flock_c)?;
            inode_file.set_advisory_lock(&lock)?;
            0
        }
    };
    Ok(ret)
}

#[derive(Debug)]
pub enum FcntlCmd<'a> {
    /// Duplicate the file descriptor fd using the lowest-numbered available
    /// file descriptor greater than or equal to arg.
    DupFd(FileDesc),
    /// As for `DupFd`, but additionally set the close-on-exec flag for the
    /// duplicate file descriptor.
    DupFdCloexec(FileDesc),
    /// Get the file descriptor flags to be close-on-exec or not
    GetFd(),
    /// Set the file descriptor to be close-on-exec or not
    SetFd(bool),
    /// Get the file status flags and access mode
    GetFl(),
    /// Set the file status flags
    SetFl(StatusFlags),
    /// Test a file advisory record lock
    GetLk(&'a mut flock_c),
    /// Acquire or release a file advisory record lock
    SetLk(&'a flock_c),
}

impl<'a> FcntlCmd<'a> {
    #[deny(unreachable_patterns)]
    pub fn from_raw(cmd: u32, arg: u64) -> Result<FcntlCmd<'a>> {
        Ok(match cmd as c_int {
            libc::F_DUPFD => FcntlCmd::DupFd(arg as FileDesc),
            libc::F_DUPFD_CLOEXEC => FcntlCmd::DupFdCloexec(arg as FileDesc),
            libc::F_GETFD => FcntlCmd::GetFd(),
            libc::F_SETFD => {
                let is_close_on_spawn = (arg as i32 & libc::FD_CLOEXEC) != 0;
                FcntlCmd::SetFd(is_close_on_spawn)
            }
            libc::F_GETFL => FcntlCmd::GetFl(),
            libc::F_SETFL => {
                let status_flags = StatusFlags::from_bits_truncate(arg as u32);
                FcntlCmd::SetFl(status_flags)
            }
            libc::F_GETLK => {
                let flock_mut_ptr = arg as *mut flock_c;
                from_user::check_mut_ptr(flock_mut_ptr)?;
                let flock_mut_c = unsafe { &mut *flock_mut_ptr };
                FcntlCmd::GetLk(flock_mut_c)
            }
            libc::F_SETLK => {
                let flock_ptr = arg as *const flock_c;
                from_user::check_ptr(flock_ptr)?;
                let flock_c = unsafe { &*flock_ptr };
                FcntlCmd::SetLk(flock_c)
            }
            _ => return_errno!(EINVAL, "unsupported command"),
        })
    }
}
