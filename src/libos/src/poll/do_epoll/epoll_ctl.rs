use super::{EpollEvent, EpollFlags};
use crate::prelude::*;

/// An epoll control command.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EpollCtl {
    Add(FileDesc, EpollEvent, EpollFlags),
    Del(FileDesc),
    Mod(FileDesc, EpollEvent, EpollFlags),
}
