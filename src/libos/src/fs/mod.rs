use super::*;

use process;
use std;
use std::any::Any;
use std::boxed::Box;
use std::fmt;
use std::io::{Read, Seek, Write};
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use util::slice_ext::{SliceAsMutPtrAndLen, SliceAsPtrAndLen};

pub use async_io::event::{Events, Observer, Pollee, Poller};
pub use async_io::file::{
    AccessMode, CreationFlags, File, FileRange, RangeLock, RangeLockBuilder, RangeLockList,
    RangeLockType, StatusFlags, OFFSET_MAX,
};
pub use async_io::fs::{
    DirentWriter, DirentWriterContext, FallocateFlags, FallocateMode, FileMode, FileSystem,
    FileType, FsError, FsInfo, INode, Metadata, MountFlags, SeekFrom, StatBuf, StatFlags, StatMode,
    Timespec, UmountFlags, PATH_MAX,
};
pub use async_io::ioctl::IoctlCmd;

/*pub use self::file_ops::{
    occlum_ocall_ioctl, AccessMode, BuiltinIoctlNum, CreationFlags, FileMode, Flock, FlockType,
    IfConf, IoctlCmd, Stat, StatusFlags, StructuredIoctlArgType, StructuredIoctlNum,
};*/

use crate::config::ConfigMount;

pub use self::disk_file::DiskFile;
pub use self::event_file::{EventFile, EventFileFlags};
pub use self::file_handle::{FileHandle as FileRef, WeakFileHandle as WeakFileRef};
pub use self::file_table::{FileDesc, FileTable};
pub use self::fs_ops::Statfs;
pub use self::fs_view::FsView;
pub use self::fspath::{FsPath, AT_FDCWD};
pub use self::host_fd::HostFd;
pub use self::inode_file::{INodeExt, INodeFile, InodeFile};
pub use self::rootfs::ROOT_FS;
pub use self::stdio::{HostStdioFds, StdinFile, StdoutFile};
pub use self::syscalls::*;

mod builtin_disk;
mod dev_fs;
mod disk_file;
mod event_file;
// TODO: remove the file
//mod file;
mod file_handle;
pub mod file_ops;
mod file_table;
mod fs_ops;
mod fs_view;
mod fspath;
mod host_fd;
mod hostfs;
mod inode_file;
mod pipe;
mod procfs;
mod rootfs;
mod sefs;
mod stdio;
mod syscalls;

/// Split a `path` str to `(dir_path, base_name)`
fn split_path(path: &str) -> (&str, &str) {
    let mut split = path.trim_end_matches('/').rsplitn(2, '/');
    let base_name = split.next().unwrap();
    let mut dir_path = split.next().unwrap_or(".");
    if dir_path == "" {
        dir_path = "/";
    }
    (dir_path, base_name)
}
