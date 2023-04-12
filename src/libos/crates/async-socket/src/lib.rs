//! Socket APIs backed by the host Linux OS.

// TODO: how to force an async I/O operation return?
// When we want to force exit a process,

#![feature(stmt_expr_attributes)]
#![feature(new_uninit)]
#![feature(raw_ref_op)]
#![cfg_attr(feature = "sgx", no_std)]

#[macro_use]
extern crate log;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_types;
#[macro_use]
extern crate lazy_static;

#[macro_use]
mod prelude;
mod common;
mod datagram;
pub mod ioctl;
mod runtime;
pub mod sockopt;
mod stream;
mod util;

pub use self::common::do_unlink;
pub use self::datagram::{DatagramSocket, NetlinkSocket};
pub use self::runtime::Runtime;
pub use self::stream::StreamSocket;
