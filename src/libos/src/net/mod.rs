use super::*;
use std;
use untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSlice, UntrustedSliceAlloc};

pub use self::io_multiplexing::{
    clear_notifier_status, notify_thread, wait_for_notification, EpollEvent, IoEvent, PollEvent,
    PollEventFlags, PollFd, THREAD_NOTIFIERS,
};
pub use self::socket::{
    socketpair, unix_socket, AsUnixSocket, Domain, HostSocket, HostSocketType, Iovs, IovsMut,
    RawAddr, SliceAsLibcIovec, UnixAddr,
};
pub use self::syscalls::*;

mod io_multiplexing;
pub(crate) mod socket;
mod syscalls;

pub use self::syscalls::*;
