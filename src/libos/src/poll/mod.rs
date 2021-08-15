mod do_epoll;
mod do_poll;
pub mod syscalls;

pub use self::do_epoll::{EpollCtl, EpollEvent, EpollFile, EpollFlags};
