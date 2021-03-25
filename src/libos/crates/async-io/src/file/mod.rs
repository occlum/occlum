mod flags;
mod handle;
mod kinds;

pub use self::flags::{AccessMode, StatusFlags};
pub use self::handle::FileHandle;
pub use self::kinds::{
    pollable::{Async, PollableFile},
    sync::SyncFile,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeekFrom {
    Start(usize),
    End(usize),
    Current(isize),
}
