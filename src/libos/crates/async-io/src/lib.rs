#![cfg_attr(feature = "sgx", no_std)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;

pub mod file;
pub mod fs;
pub mod ioctl;
pub mod poll;
pub mod prelude;
pub mod util;
