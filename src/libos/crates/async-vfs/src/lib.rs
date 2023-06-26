#![cfg_attr(feature = "sgx", no_std)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

pub use dentry::Dentry;
pub use fs::AsyncFileSystem;
pub use inode::AsyncInode;

mod dentry;
mod fs;
mod inode;
mod prelude;
