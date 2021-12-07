//! This crate provide the abstractions for block devices.
#![cfg_attr(not(test), no_std)]
#![feature(new_uninit)]
#![feature(slice_fill)]

extern crate alloc;

pub mod block_buf;
pub mod block_device;
//mod block_device_ext;
pub mod block_io;
pub mod mem_disk;
mod prelude;

pub const BLOCK_SIZE: usize = 4096;

pub use self::block_buf::BlockBuf;
pub use self::block_device::BlockDevice;
pub use self::block_io::{BioCompletionCallback, BioReq, BioResp, BioSubmission, BioType};

pub type BlockId = usize;
