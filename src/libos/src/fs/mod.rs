use super::*;

use process;
use std;
use std::any::Any;
use std::fmt;
use std::io::{Read, Seek, Write};
use std::mem::MaybeUninit;
use std::path::Path;
use untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen};

pub use async_io::file::{
    AccessMode, CreationFlags, FileHandle as FileRef, PollableFile, SeekFrom, StatusFlags, SyncFile,
};
pub use async_io::fs::{
    FileMode, FileSystem, FileType, FsError, INode, Metadata, StatBuf, StatFlags, StatMode,
    Timespec, PATH_MAX,
};
pub use async_io::poll::Events;

/*pub use self::file_ops::{
    occlum_ocall_ioctl, AccessMode, BuiltinIoctlNum, CreationFlags, FileMode, Flock, FlockType,
    IfConf, IoctlCmd, Stat, StatusFlags, StructuredIoctlArgType, StructuredIoctlNum,
};*/
pub use self::event_file::{EventFile, EventFileFlags};
pub use self::file_table::{FileDesc, FileTable};
pub use self::fs_view::FsView;
pub use self::host_fd::HostFd;
pub use self::inode_file::{INodeExt, INodeFile};
pub use self::rootfs::ROOT_INODE;
pub use self::stdio::{HostStdioFds, StdinFile, StdoutFile};
pub use self::syscalls::*;

mod event_file;
//mod dev_fs;
// TODO: remove the file
//mod file;
mod file_ops;
mod file_table;
mod fs_ops;
mod fs_view;
mod host_fd;
mod hostfs;
mod inode_file;
mod pipe;
//mod procfs;
mod rootfs;
mod sefs;
mod stdio;
mod syscalls;

/// Split a `path` str to `(base_path, file_name)`
fn split_path(path: &str) -> (&str, &str) {
    let mut split = path.trim_end_matches('/').rsplitn(2, '/');
    let file_name = split.next().unwrap();
    let mut dir_path = split.next().unwrap_or(".");
    if dir_path == "" {
        dir_path = "/";
    }
    (dir_path, file_name)
}
