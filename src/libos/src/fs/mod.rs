use super::*;

use process;
use rcore_fs::vfs::{
    DirentVisitor, FileSystem, FileType, FsError, INode, Metadata, Timespec, PATH_MAX,
};
use std;
use std::any::Any;
use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::MaybeUninit;
use std::path::Path;
use untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen};

use crate::config::ConfigMount;

pub use self::event_file::{AsEvent, EventCreationFlags, EventFile};
pub use self::events::{AtomicIoEvents, IoEvents, IoNotifier};
pub use self::file::{File, FileRef};
pub use self::file_ops::{
    occlum_ocall_ioctl, utimbuf_t, AccessMode, BuiltinIoctlNum, CreationFlags, FallocateFlags,
    FileMode, GetIfConf, GetIfReqWithRawCmd, GetReadBufLen, GetWinSize, IfConf, IoctlCmd,
    IoctlRawCmd, NonBuiltinIoctlCmd, SetNonBlocking, SetWinSize, Stat, StatusFlags,
    StructuredIoctlArgType, StructuredIoctlNum, TcGets, TcSets, STATUS_FLAGS_MASK,
};
pub use self::file_table::{FileDesc, FileTable, FileTableEvent, FileTableNotifier};
pub use self::fs_ops::Statfs;
pub use self::fs_view::FsView;
pub use self::host_fd::HostFd;
pub use self::inode_file::{AsINodeFile, INodeExt, INodeFile};
pub use self::locks::flock::{Flock, FlockList, FlockOps, FlockType};
pub use self::locks::range_lock::{
    FileRange, RangeLock, RangeLockBuilder, RangeLockList, RangeLockType, OFFSET_MAX,
};
pub use self::pipe::PipeType;
pub use self::rootfs::{ROOT_FS, SEFS_MANAGER};
pub use self::stdio::{HostStdioFds, StdinFile, StdoutFile};
pub use self::syscalls::*;
pub use self::timer_file::{AsTimer, TimerCreationFlags, TimerFile};

pub mod channel;
mod dev_fs;
mod event_file;
mod events;
mod file;
mod file_ops;
mod file_table;
mod fs_ops;
mod fs_view;
mod host_fd;
mod hostfs;
mod inode_file;
mod locks;
mod pipe;
mod procfs;
mod rootfs;
mod sefs;
mod stdio;
mod syscalls;
mod timer_file;

/// Split a `path` to (`dir_path`, `file_name`).
///
/// The `dir_path` must be a directory.
///
/// The `file_name` is the last component. It can be suffixed by "/".
///
/// Example:
///
/// The path "/dir/file/" will be split to ("/dir", "file/").
fn split_path(path: &str) -> (&str, &str) {
    let file_name = path
        .split_inclusive('/')
        .filter(|&x| x != "/")
        .last()
        .unwrap_or(".");

    let mut split = path.trim_end_matches('/').rsplitn(2, '/');
    let dir_path = if split.next().unwrap().is_empty() {
        "/"
    } else {
        let mut dir = split.next().unwrap_or(".").trim_end_matches('/');
        if dir.is_empty() {
            dir = "/";
        }
        dir
    };

    (dir_path, file_name)
}

// Linux uses 40 as the upper limit for resolving symbolic links,
// so Occlum use it as a reasonable value
pub const MAX_SYMLINKS: usize = 40;
