use super::*;

use process;
use rcore_fs::vfs::{FileSystem, FileType, FsError, INode, Metadata, Timespec};
use std;
use std::any::Any;
use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::MaybeUninit;
use std::path::Path;
use untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen};

pub use self::dev_fs::AsDevRandom;
pub use self::event_file::{AsEvent, EventFile};
pub use self::file::{File, FileRef};
pub use self::file_ops::{AccessMode, CreationFlags, FileMode, Stat, StatusFlags};
pub use self::file_ops::{Flock, FlockType};
pub use self::file_ops::{IoctlCmd, StructuredIoctlArgType, StructuredIoctlNum};
pub use self::file_table::{FileDesc, FileTable};
pub use self::fs_view::FsView;
pub use self::inode_file::{AsINodeFile, INodeExt, INodeFile};
pub use self::pipe::Pipe;
pub use self::rootfs::ROOT_INODE;
pub use self::stdio::{HostStdioFds, StdinFile, StdoutFile};
pub use self::syscalls::*;

mod dev_fs;
mod event_file;
mod file;
mod file_ops;
mod file_table;
mod fs_ops;
mod fs_view;
mod hostfs;
mod inode_file;
mod pipe;
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
