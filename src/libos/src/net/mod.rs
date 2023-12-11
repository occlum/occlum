use super::*;
use std;
use untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSlice, UntrustedSliceAlloc};

pub use self::io_multiplexing::{
    clear_notifier_status, notify_thread, wait_for_notification, EpollEvent, IoEvent, PollEvent,
    PollEventFlags, PollFd, THREAD_NOTIFIERS,
};

pub use self::ocall_socket::{
    socketpair, unix_socket, AddressFamily, AsUnixSocket, HostSocket, HostSocketType, Iovs,
    IovsMut, MsgHdr, MsgHdrMut, SliceAsLibcIovec, SockAddr, UnixAddr,
};
pub use self::syscalls::*;

pub use self::addr::*;

mod addr;
mod io_multiplexing;
mod ocall_socket;
mod socket_file;
mod socket_file_impl;
mod sockopt;
mod syscalls;

pub use self::syscalls::*;
