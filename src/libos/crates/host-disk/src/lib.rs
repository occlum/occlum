//! Virtual disks backed by files on the host Linux kernel.

#![cfg_attr(feature = "sgx", no_std)]
#![feature(new_uninit)]
#![feature(get_mut_unchecked)]

#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_types;

mod host_disk;
mod io_uring_disk;
mod open_options;
mod prelude;
mod sync_io_disk;

pub use self::host_disk::HostDisk;
pub use self::io_uring_disk::IoUringDisk;
pub use self::open_options::OpenOptions;
pub use self::sync_io_disk::SyncIoDisk;
