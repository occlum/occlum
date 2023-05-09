#![cfg_attr(feature = "sgx", no_std)]
#![feature(new_uninit)]
#![feature(slice_group_by)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;

#[macro_use]
extern crate log;

extern crate lru;

// Export SimpleFileSystem
pub use fs::AsyncSimpleFS;

mod fs;
mod metadata;
mod prelude;
mod storage;
mod utils;

#[cfg(test)]
mod tests;
