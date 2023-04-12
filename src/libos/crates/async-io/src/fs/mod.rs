mod fallocate_flags;
mod file_mode;
mod mount_flags;
mod stat_buf;

pub use rcore_fs::vfs::{
    AnyExt, DirentWriter, DirentWriterContext, Extension, FallocateMode, FileSystem, FileType,
    FsError, FsInfo, FsMac, INode, Metadata, Timespec, PATH_MAX,
};

pub use self::fallocate_flags::FallocateFlags;
pub use self::file_mode::FileMode;
pub use self::mount_flags::{MountFlags, UmountFlags};
pub use self::stat_buf::{StatBuf, StatFlags, StatMode};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}
