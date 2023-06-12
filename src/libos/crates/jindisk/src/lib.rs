//! This crate provides the abstractions for JinDisk.
//!
//! JinDisk is a log-structured secure block device for TEEs, allowing file systems to
//! be stacked upon it for transparent I/O protection. This crate is an Intel SGX version for Occlum
//! where JinDisk serves as a trusted logical block device within LibOS.
//!
//! Guarantees: confidentiality, integrity, freshness, anonymity, consistency, (flush) atomicity.
//!
//! JinDisk uses a secure data log to organize encrypted data blocks into segments (out-of-place updates).
//! It keeps the mapping between logical block address (Lba) and host block address (Hba) in TEE.
//!
//! It designs the secure index and secure journal to jointly protect the out-of-place updated on-disk data.
//! The secure index is an enhanced LSM-tree which integrates an MHT with a B+ tree
//! for each of its immutable disk components to index the data log securely.
//! The secure journal is a chain of records which summarizes the persistence information of the
//! data log and the index to achieve crash consistency.
//!
//! JinDisk provides four standard block I/O command as follows:
//!
//! - `read(lba: Lba, buf: &mut [u8])`
//! - `write(lba: Lba, buf: &[u8])`
//! - `flush()`
//! - `discard(lbas: &[Lba])`
//!
//! JinDisk protects all block I/O through these operations transparently.
//!
//! # Usage example
//!
//! ```rust
//! use jindisk::{DefaultCryptor, GiB, Hba, Lba, JinDisk};
//! use block_device::BLOCK_SIZE;
//! use sgx_disk::{HostDisk, SyncIoDisk};
//!
//! let total_blocks = 1 * GiB / BLOCK_SIZE;
//! let path = "jindisk.image";
//!
//! // Create underlying disk
//! let sync_disk = SyncIoDisk::create(&path, total_blocks).unwrap();
//! let root_key = DefaultCryptor::gen_random_key();
//!
//! // Create JinDisk
//! let jindisk = JinDisk::create(Arc::new(sync_disk), root_key);
//!
//! let total_bytes = jindisk.total_bytes();
//! let content = 5u8;
//! let offset = Lba::new(0).to_offset();
//!
//! // Write block contents to JinDisk
//! let wbuf = [content; BLOCK_SIZE];
//! jindisk.write(offset, &wbuf).await?;
//!
//! // Read block contents from JinDisk
//! let mut rbuf = [0u8; BLOCK_SIZE];
//! jindisk.read(offset, &mut rbuf).await?;
//!
//! // Sync all data to storage medium
//! jindisk.sync().await?;
//!
//! assert_eq!(rbuf, wbuf);
//! ```
//!
#![cfg_attr(feature = "sgx", no_std)]
#![feature(const_fn_trait_bound)]
#![feature(drain_filter)]
#![feature(in_band_lifetimes)]
#![feature(into_future)]
#![feature(is_sorted)]
#![feature(new_uninit)]
#![feature(slice_group_by)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_alloc;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_rand;
#[cfg(feature = "sgx")]
extern crate sgx_tcrypto;

#[macro_use]
extern crate log;

mod checkpoint;
mod config;
mod data;
mod index;
mod jindisk;
mod journal;
mod prelude;
mod superblock;
mod util;

use self::checkpoint::Checkpoint;
pub use self::config::{GiB, Hba, KiB, Lba, MiB, SEGMENT_SIZE};
use self::data::{Cleaner, DataCache};
use self::index::{bit::Bit, LsmTree, Record};
pub use self::jindisk::JinDisk;
pub use self::superblock::SuperBlock;
pub use self::util::cryption::{Cryption, DefaultCryptor};
use self::util::serialize::Serialize;

// This crate assumes the machine is 64-bit to use u64 and usize interchangeably.
use static_assertions::assert_eq_size;
assert_eq_size!(usize, u64);
