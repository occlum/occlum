mod flags;
mod pollable;

pub use self::flags::{AccessMode, CreationFlags, StatusFlags};
pub use self::pollable::{Async, PollableFile};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}
