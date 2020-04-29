use super::dev_fs::{DevNull, DevRandom, DevSgx, DevZero};
use super::*;
use process::Process;

pub use self::access::{do_faccessat, AccessibilityCheckFlags, AccessibilityCheckMode};
pub use self::chmod::{do_chmod, do_fchmod, FileMode};
pub use self::chown::{do_chown, do_fchown, do_lchown};
pub use self::close::do_close;
pub use self::dirent::do_getdents64;
pub use self::dirfd::{get_dir_path, DirFd};
pub use self::dup::{do_dup, do_dup2, do_dup3};
pub use self::fcntl::{do_fcntl, FcntlCmd};
pub use self::file_flags::{AccessMode, CreationFlags, StatusFlags};
pub use self::flock::{Flock, FlockType};
pub use self::fsync::{do_fdatasync, do_fsync};
pub use self::ioctl::{do_ioctl, IoctlCmd, StructuredIoctlArgType, StructuredIoctlNum};
pub use self::link::do_link;
pub use self::lseek::do_lseek;
pub use self::mkdir::do_mkdir;
pub use self::open::do_openat;
pub use self::read::{do_pread, do_read, do_readv};
pub use self::rename::do_rename;
pub use self::rmdir::do_rmdir;
pub use self::sendfile::do_sendfile;
pub use self::stat::{do_fstat, do_fstatat, do_lstat, Stat, StatFlags};
pub use self::symlink::do_readlink;
pub use self::truncate::{do_ftruncate, do_truncate};
pub use self::unlink::do_unlink;
pub use self::write::{do_pwrite, do_write, do_writev};

mod access;
mod chmod;
mod chown;
mod close;
mod dirent;
mod dirfd;
mod dup;
mod fcntl;
mod file_flags;
mod flock;
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
