mod file_mode;
mod stat_buf;

pub use rcore_fs::vfs::{FileSystem, FileType, FsError, INode, Metadata, Timespec, PATH_MAX};

pub use self::file_mode::FileMode;
pub use self::stat_buf::{StatBuf, StatFlags, StatMode};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}
