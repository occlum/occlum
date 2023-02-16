//! This crate provide the abstractions for block devices.

// TODO: add O_DIRECT flag?

#![cfg_attr(not(test), no_std)]
#![feature(new_uninit)]

#[macro_use]
extern crate alloc;

pub mod block_buf;
pub mod block_device;
pub mod block_device_as_file;
pub mod block_id;
pub mod block_io;
pub mod mem_disk;
mod prelude;
pub mod util;

pub const BLOCK_SIZE: usize = 4096;
pub const BLOCK_SIZE_LOG2: usize = 12;

pub use self::block_buf::BlockBuf;
pub use self::block_device::BlockDevice;
pub use self::block_device_as_file::BlockDeviceAsFile;
pub use self::block_id::{Bid, RawBid};
pub use self::block_io::{
    BioReq, BioReqBuilder, BioReqOnCompleteFn, BioReqOnDropFn, BioResp, BioSubmission, BioType,
};
pub use self::util::anymap::{Any, AnyMap};
pub use self::util::block_range::{BlockRange, BlockRangeIter};

// This crate assumes the machine is 64-bit to use u64 and usize interchangeably.
use static_assertions::assert_eq_size;
assert_eq_size!(usize, u64);
