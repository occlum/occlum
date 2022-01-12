//! Virtual disks backed by files on the host Linux kernel.

#![cfg_attr(feature = "sgx", no_std)]
#![feature(new_uninit)]
#![feature(get_mut_unchecked)]

#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_tcrypto;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_types;
#[macro_use]
extern crate log;

mod host_disk;
mod io_uring_disk;
mod open_options;
mod prelude;
#[cfg(feature = "sgx")]
mod sync_enc_io_disk;
mod sync_io_disk;
#[cfg(feature = "sgx")]
mod sync_pfs;

pub use self::host_disk::HostDisk;
pub use self::io_uring_disk::{IoUringDisk, IoUringProvider};
pub use self::open_options::OpenOptions;
#[cfg(feature = "sgx")]
pub use self::sync_enc_io_disk::SyncEncIoDisk;
pub use self::sync_io_disk::SyncIoDisk;
#[cfg(feature = "sgx")]
pub use self::sync_pfs::SyncSgxDisk;
