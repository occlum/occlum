//! Configurable parameters.
use crate::index::bit::{BIT_SIZE_ON_DISK, MAX_RECORD_NUM_PER_BIT};
use crate::prelude::*;
use crate::SuperBlock;

use std::fmt::{self, Debug};
use std::mem::size_of;
use std::time::Duration;

// Logical Block Address
pub type Lba = Bid;
// Host Block Address
pub type Hba = Bid;

pub const B: usize = 1;
#[allow(non_upper_case_globals)]
pub const KiB: usize = 1024 * B;
#[allow(non_upper_case_globals)]
pub const MiB: usize = 1024 * KiB;
#[allow(non_upper_case_globals)]
pub const GiB: usize = 1024 * MiB;

/// # SuperBlock
// The proportion of data region (TBD)
pub const DATA_PROPORTION: f32 = 0.95;

// Magic number (TBD)
pub const MAGIC_NUMBER: u32 = 0x1130_0821;

pub const SUPER_BLOCK_SIZE: usize = size_of::<SuperBlock>();
pub const SUPER_BLOCK_REGION_ADDR: Hba = Hba::new(0);
// Index segment size (equals to on-disk size of a BIT)
pub const INDEX_SEGMENT_SIZE: usize = BIT_SIZE_ON_DISK;

/// # JinDisk
// Batch read threshold (Range query trigger condition)
pub const BATCH_READ_THRESHOLD: usize = 2 * BLOCK_SIZE;

/// # Data
// Segment Id
#[allow(unused)]
pub type SegmentId = usize;
// Segment size (1024 blocks)
pub const SEGMENT_SIZE: usize = 4 * MiB;
pub const NUM_BLOCKS_PER_SEGMENT: usize = SEGMENT_SIZE / BLOCK_SIZE;
// Number of blocks in one segment buffer
pub const SEGMENT_BUFFER_CAPACITY: usize = NUM_BLOCKS_PER_SEGMENT;
// Number of segment buffers in pool
pub const BUFFER_POOL_CAPACITY: usize = 16;

/// # Garbage collection
pub const GC_WATERMARK: usize = 16;
pub const GC_BACKGROUND_PERIOD: Duration = Duration::from_secs(5);

/// # Index
/// ## MemTable
pub const MAX_MEM_TABLE_CAPACITY: usize = DATA_SIZE_PER_BIT / BLOCK_SIZE;

/// ## LsmTree
// Data size that one BIT can manage
// TODO: Make BIT amount and shape tunable
pub const DATA_SIZE_PER_BIT: usize = MAX_RECORD_NUM_PER_BIT * BLOCK_SIZE;
pub const MAX_LEVEL0_BIT_NUM: usize = 1;

/// ## Record
// Negative hba/lba (Used for delayed reclamation and block discard)
pub const NEGATIVE_HBA: Hba = Hba::new(RawBid::MAX as _);
pub const NEGATIVE_LBA: Lba = Lba::new(RawBid::MAX as _);

struct JinDiskConfig;
impl Debug for JinDiskConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JinDiskConfig")
            .field("SEGMENT_BUFFER_CAPACITY", &SEGMENT_BUFFER_CAPACITY)
            .field("BUFFER_POOL_CAPACITY", &BUFFER_POOL_CAPACITY)
            .field("MAX_MEM_TABLE_CAPACITY", &MAX_MEM_TABLE_CAPACITY)
            .field("DATA_SIZE_PER_BIT", &DATA_SIZE_PER_BIT)
            .field("MAX_LEVEL0_BIT_NUM", &MAX_LEVEL0_BIT_NUM)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_configs() {
        println!("{:#?}", JinDiskConfig);
    }
}
