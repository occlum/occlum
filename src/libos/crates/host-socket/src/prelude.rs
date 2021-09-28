// Convenient reexports for internal uses.
pub(crate) use errno::prelude::*;
pub(crate) use std::sync::Arc;

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        pub(crate) use std::prelude::v1::*;
        pub(crate) use std::sync::{SgxMutex as Mutex, SgxRwLock as RwLock, SgxMutexGuard as MutexGuard};
    } else {
        pub(crate) use std::sync::{Mutex, MutexGuard, RwLock};
    }
}

// Convenient type alises for internal uses.
pub(crate) type HostFd = u32;

pub(crate) use async_io::event::{Events, Observer, Pollee, Poller};
pub(crate) use async_io::file::StatusFlags;
pub(crate) use async_io::ioctl::IoctlCmd;
pub(crate) use async_io::socket::{Addr, Domain, Type};

macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);

        match &name[..name.len() - 3].rfind(':') {
            Some(pos) => &name[pos + 1..name.len() - 3],
            None => &name[..name.len() - 3],
        }
    }};
}

macro_rules! debug_trace {
    () => {
        println!(
            "> Function = {}, Line = {}, File = {}",
            function!(),
            line!(),
            file!()
        )
    };
}

// return Err(errno) if libc return -1
macro_rules! try_libc {
    ($ret: expr) => {{
        let ret = unsafe { $ret };
        if ret < 0 {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sgx")] {
                    let errno = libc::errno();
                } else {
                    let errno = unsafe { libc::__errno_location() };
                }
            }
            return_errno!(Errno::from(errno as u32), "libc error");
        }
        ret
    }};
}
