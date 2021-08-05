use async_io::event::Events;

use crate::prelude::*;

/// An epoll event.
///
/// This could be used as either an input of epoll ctl or an output of epoll wait.
/// The memory layout is compatible with that of C's struct epoll_event.
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
pub struct EpollEvent {
    /// I/O events.
    ///
    /// When `EpollEvent` is used as inputs, this is treated as a mask of events.
    /// When `EpollEvent` is used as outputs, this is the active events.
    pub events: Events,
    /// A 64-bit, user-given data.
    pub user_data: u64,
}

impl EpollEvent {
    /// Create a new epoll event.
    pub fn new(events: Events, user_data: u64) -> Self {
        Self { events, user_data }
    }
}

impl From<&libc::epoll_event> for EpollEvent {
    fn from(c_event: &libc::epoll_event) -> Self {
        Self {
            events: Events::from_bits_truncate(c_event.events as u32),
            user_data: c_event.u64,
        }
    }
}

impl From<&EpollEvent> for libc::epoll_event {
    fn from(ep_event: &EpollEvent) -> Self {
        libc::epoll_event {
            events: ep_event.events.bits() as u32,
            u64: ep_event.user_data,
        }
    }
}
