//! Block id.
use core::ops::{Add, Sub};

use crate::prelude::*;
use errno::return_errno;

pub type RawBid = u64;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bid(RawBid);

impl Bid {
    pub const fn new(raw_bid: RawBid) -> Self {
        debug_assert!(
            BLOCK_SIZE.saturating_mul(raw_bid as _) <= usize::MAX,
            "block id is too big"
        );
        Self(raw_bid)
    }

    pub const fn from_byte_offset(offset: usize) -> Self {
        Self((offset / BLOCK_SIZE) as _)
    }

    pub fn from_byte_offset_aligned(offset: usize) -> Result<Self> {
        Self::check_align(offset)?;
        Ok(Self((offset / BLOCK_SIZE) as _))
    }

    pub fn to_raw(self) -> RawBid {
        self.0
    }

    pub fn to_offset(self) -> usize {
        (self.0 as usize) * BLOCK_SIZE
    }

    fn check_align(offset: usize) -> Result<()> {
        if offset % BLOCK_SIZE != 0 {
            return_errno!(EINVAL, "offset not aligned with block size")
        }
        Ok(())
    }
}

impl Add<u64> for Bid {
    type Output = Self;

    fn add(self, other: u64) -> Self::Output {
        Self(self.0 + other)
    }
}

impl Sub<u64> for Bid {
    type Output = Self;

    fn sub(self, other: u64) -> Self::Output {
        Self(self.0 - other)
    }
}

impl Into<RawBid> for Bid {
    fn into(self) -> RawBid {
        self.to_raw()
    }
}
