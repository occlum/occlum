//! Socket APIs backed by the host Linux OS.

// TODO: how to async I/O request to return?
// When we want to force exit a process,

cfg_if::cfg_if! {
    if #[cfg(feature="sgx")] {
        extern crate sgx_tstd as std;
        extern crate sgx_libc as libc;
    }
}

#[macro_use]
mod prelude;
mod runtime;
mod stream;
mod util;

pub use self::runtime::Runtime;
pub use self::stream::StreamSocket;
