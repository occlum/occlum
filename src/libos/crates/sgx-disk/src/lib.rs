//! Virtual disks that can be used inside SGX.
//!
//! Currently, there are four types of disks
//! * `SyncIoDisk` is an untrusted disk backed by a file on the host Linux.
//! It performs normal sync I/O to the underlying file.
//! * `IoUringDisk` is an untrusted disk backed by a file on the host Linux.
//! It performs io_uring-powered async I/O to the underlying file.
//! * `CryptDisk` is a "decorator" disk that adds a layer of encryption atop
//! another disk.
//! * `PfsDisk` is a secure disk backed by Intel SGX Protected File System Library
//! (SGX-PFS).

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

mod crypt_disk;
pub mod host_disk;
#[cfg(feature = "sgx")]
pub mod pfs_disk;
mod prelude;

pub use self::crypt_disk::CryptDisk;
pub use self::host_disk::{HostDisk, IoUringDisk, IoUringProvider, SyncIoDisk};
#[cfg(feature = "sgx")]
pub use self::pfs_disk::PfsDisk;
