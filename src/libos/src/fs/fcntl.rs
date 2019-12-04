use super::*;
use process::FileTableRef;

#[derive(Debug)]
pub enum FcntlCmd {
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
}

impl FcntlCmd {
    #[deny(unreachable_patterns)]
    pub fn from_raw(cmd: u32, arg: u64) -> Result<FcntlCmd> {
        Ok(match cmd as c_int {
            libc::F_DUPFD => FcntlCmd::DupFd(arg as FileDesc),
            libc::F_DUPFD_CLOEXEC => FcntlCmd::DupFdCloexec(arg as FileDesc),
            libc::F_GETFD => FcntlCmd::GetFd(),
            libc::F_SETFD => FcntlCmd::SetFd(arg as u32),
            libc::F_GETFL => FcntlCmd::GetFl(),
            libc::F_SETFL => FcntlCmd::SetFl(arg as u32),
            _ => return_errno!(EINVAL, "unsupported command"),
        })
    }
}

pub fn do_fcntl(file_table_ref: &FileTableRef, fd: FileDesc, cmd: &FcntlCmd) -> Result<isize> {
    let mut file_table = file_table_ref.lock().unwrap();
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
            entry.set_close_on_spawn((fd_flags & libc::FD_CLOEXEC as u32) != 0);
            0
        }
        FcntlCmd::GetFl() => {
            let file = file_table.get(fd)?;
            let status_flags = file.get_status_flags()?;
            let access_mode = file.get_access_mode()?;
            (status_flags.bits() | access_mode as u32) as isize
        }
        FcntlCmd::SetFl(flags) => {
            let file = file_table.get(fd)?;
            let status_flags = StatusFlags::from_bits_truncate(*flags);
            file.set_status_flags(status_flags)?;
            0
        }
    };
    Ok(ret)
}
