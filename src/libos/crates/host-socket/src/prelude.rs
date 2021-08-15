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
pub(crate) use async_io::socket::{Addr, Domain};

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
