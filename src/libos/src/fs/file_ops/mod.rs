use super::dev_fs::{DevNull, DevRandom, DevSgx, DevZero};
use super::*;
use process::Process;

pub use self::access::{
    do_access, do_faccessat, AccessibilityCheckFlags, AccessibilityCheckMode, AT_FDCWD,
};
pub use self::chdir::do_chdir;
pub use self::close::do_close;
pub use self::dirent::do_getdents64;
pub use self::dup::{do_dup, do_dup2, do_dup3};
pub use self::fcntl::{do_fcntl, FcntlCmd};
pub use self::file_flags::{AccessMode, CreationFlags, StatusFlags};
pub use self::fsync::{do_fdatasync, do_fsync};
pub use self::ioctl::{do_ioctl, IoctlCmd, StructuredIoctlArgType, StructuredIoctlNum};
pub use self::link::do_link;
pub use self::lseek::do_lseek;
pub use self::mkdir::do_mkdir;
pub use self::open::do_open;
pub use self::read::{do_pread, do_read, do_readv};
pub use self::rename::do_rename;
pub use self::rmdir::do_rmdir;
pub use self::sendfile::do_sendfile;
pub use self::stat::{do_fstat, do_lstat, do_stat, Stat};
pub use self::symlink::do_readlink;
pub use self::truncate::{do_ftruncate, do_truncate};
pub use self::unlink::do_unlink;
pub use self::write::{do_pwrite, do_write, do_writev};

mod access;
mod chdir;
mod close;
mod dirent;
mod dup;
mod fcntl;
mod file_flags;
mod fsync;
mod ioctl;
mod link;
mod lseek;
mod mkdir;
mod open;
mod read;
mod rename;
mod rmdir;
mod sendfile;
mod stat;
mod symlink;
mod truncate;
mod unlink;
mod write;

impl Process {
    /// Open a file on the process. But DO NOT add it to file table.
    pub fn open_file(&self, path: &str, flags: u32, mode: u32) -> Result<Box<dyn File>> {
        if path == "/dev/null" {
            return Ok(Box::new(DevNull));
        }
        if path == "/dev/zero" {
            return Ok(Box::new(DevZero));
        }
        if path == "/dev/random" || path == "/dev/urandom" || path == "/dev/arandom" {
            return Ok(Box::new(DevRandom));
        }
        if path == "/dev/sgx" {
            return Ok(Box::new(DevSgx));
        }
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        let inode = if creation_flags.can_create() {
            let (dir_path, file_name) = split_path(&path);
            let dir_inode = self.lookup_inode(dir_path)?;
            match dir_inode.find(file_name) {
                Ok(file_inode) => {
                    if creation_flags.is_exclusive() {
                        return_errno!(EEXIST, "file exists");
                    }
                    file_inode
                }
                Err(FsError::EntryNotFound) => {
                    if !dir_inode.allow_write()? {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode.create(file_name, FileType::File, mode)?
                }
                Err(e) => return Err(Error::from(e)),
            }
        } else {
            self.lookup_inode(&path)?
        };
        let abs_path = self.convert_to_abs_path(&path);
        Ok(Box::new(INodeFile::open(inode, &abs_path, flags)?))
    }

    /// Lookup INode from the cwd of the process
    pub fn lookup_inode(&self, path: &str) -> Result<Arc<dyn INode>> {
        debug!("lookup_inode: cwd: {:?}, path: {:?}", self.get_cwd(), path);
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // absolute path
            let abs_path = path.trim_start_matches('/');
            let inode = ROOT_INODE.lookup(abs_path)?;
            Ok(inode)
        } else {
            // relative path
            let cwd = self.get_cwd().trim_start_matches('/');
            let inode = ROOT_INODE.lookup(cwd)?.lookup(path)?;
            Ok(inode)
        }
    }

    /// Convert the path to be absolute
    pub fn convert_to_abs_path(&self, path: &str) -> String {
        debug!(
            "convert_to_abs_path: cwd: {:?}, path: {:?}",
            self.get_cwd(),
            path
        );
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // path is absolute path already
            return path.to_owned();
        }
        let cwd = {
            if !self.get_cwd().ends_with("/") {
                self.get_cwd().to_owned() + "/"
            } else {
                self.get_cwd().to_owned()
            }
        };
        cwd + path
    }
}

/// Split a `path` str to `(base_path, file_name)`
pub fn split_path(path: &str) -> (&str, &str) {
    let mut split = path.trim_end_matches('/').rsplitn(2, '/');
    let file_name = split.next().unwrap();
    let mut dir_path = split.next().unwrap_or(".");
    if dir_path == "" {
        dir_path = "/";
    }
    (dir_path, file_name)
}
