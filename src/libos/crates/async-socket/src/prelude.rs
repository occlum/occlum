// Convenient reexports for internal uses.
pub(crate) use errno::prelude::*;
pub(crate) use std::sync::Arc;
pub(crate) use std::time::Duration;

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        pub(crate) use std::prelude::v1::*;
        pub(crate) use std::sync::{SgxMutex as Mutex, SgxRwLock as RwLock, SgxMutexGuard as MutexGuard};
    } else {
        pub(crate) use std::sync::{Mutex, MutexGuard, RwLock};
    }
}

// Convenient type alias for internal uses.
pub(crate) type HostFd = u32;

pub(crate) use async_io::event::{Events, Observer, Pollee, Poller};
pub(crate) use async_io::file::StatusFlags;
pub(crate) use async_io::ioctl::IoctlCmd;
pub(crate) use async_io::socket::{Addr, Domain, RecvFlags, SendFlags, Shutdown, Type};

// return Err(errno) if libc return -1
macro_rules! try_libc {
    ($ret: expr) => {{
        let ret = unsafe { $ret };
        if ret < 0 {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sgx")] {
                    let errno = libc::errno();
                } else {
                    let errno = unsafe { *libc::__errno_location() };
                }
            }
            return_errno!(Errno::from(errno as u32), "libc error");
        }
        ret
    }};
}
