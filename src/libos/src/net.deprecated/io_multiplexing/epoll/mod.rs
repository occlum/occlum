use crate::fs::IoEvents;
use crate::prelude::*;

mod epoll_file;
mod epoll_waiter;
mod host_file_epoller;

pub use self::epoll_file::{AsEpollFile, EpollFile};

/// An epoll control command.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EpollCtl {
    Add(FileDesc, EpollEvent, EpollFlags),
    Del(FileDesc),
    Mod(FileDesc, EpollEvent, EpollFlags),
}

/// An epoll control flags.
bitflags! {
    pub struct EpollFlags: u32 {
        const EXCLUSIVE      = (1 << 28);
        const WAKE_UP        = (1 << 29);
        const ONE_SHOT       = (1 << 30);
        const EDGE_TRIGGER   = (1 << 31);
    }
}

impl EpollFlags {
    pub fn from_c(c_event: &libc::epoll_event) -> Self {
        EpollFlags::from_bits_truncate(c_event.events)
    }
}

/// An epoll event.
///
/// This could be used as either an input of epoll ctl or an output of epoll wait.
// Note: the memory layout is compatible with that of C's struct epoll_event.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EpollEvent {
    mask: IoEvents,
    user_data: u64,
}

impl EpollEvent {
    pub fn new(mask: IoEvents, user_data: u64) -> Self {
        Self { mask, user_data }
    }

    pub fn mask(&self) -> IoEvents {
        self.mask
    }

    pub fn user_data(&self) -> u64 {
        self.user_data
    }

    pub fn from_c(c_event: &libc::epoll_event) -> Self {
        let mask = IoEvents::from_raw(c_event.events as u32);
        let user_data = c_event.u64;
        Self { mask, user_data }
    }

    pub fn to_c(&self) -> libc::epoll_event {
        libc::epoll_event {
            events: self.mask.bits() as u32,
            u64: self.user_data,
        }
    }
}
