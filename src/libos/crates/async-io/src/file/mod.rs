mod flags;
mod handle;
mod kinds;

pub use self::flags::{AccessMode, CreationFlags, StatusFlags};
pub use self::handle::FileHandle;
pub use self::kinds::{
    pollable::{Async, PollableFile},
    sync::SyncFile,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}
