use super::*;

pub const AT_FDCWD: i32 = -100;

#[derive(Debug)]
pub enum DirFd {
    Cwd,
    Fd(FileDesc),
}

impl DirFd {
    pub fn from_i32(fd: i32) -> Result<DirFd> {
        let dirfd = if fd >= 0 {
            DirFd::Fd(fd as FileDesc)
        } else if fd == AT_FDCWD {
            DirFd::Cwd
        } else {
            return_errno!(EINVAL, "invalid dirfd");
        };
        Ok(dirfd)
    }
}

// Get the absolute path of directory
pub fn get_dir_path(dirfd: FileDesc) -> Result<String> {
    let dir_path = {
        let current_ref = process::get_current();
        let proc = current_ref.lock().unwrap();
        let file_ref = proc.get_files().lock().unwrap().get(dirfd)?;
        if let Ok(inode_file) = file_ref.as_inode_file() {
            if inode_file.metadata()?.type_ != FileType::Dir {
                return_errno!(ENOTDIR, "not a directory");
            }
            inode_file.get_abs_path().to_owned()
        } else {
            return_errno!(EBADF, "not an inode file");
        }
    };
    Ok(dir_path)
}
