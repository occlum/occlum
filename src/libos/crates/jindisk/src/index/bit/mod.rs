//! Block Index Table.
mod builder;
mod cache;
pub mod disk_bit;
mod mem_bit;

pub(crate) use builder::BitBuilder;
pub(crate) use disk_bit::{BIT_SIZE, BIT_SIZE_ON_DISK, MAX_RECORD_NUM_PER_BIT};
pub(crate) use mem_bit::Bit;

// Unique id of a BIT (equals to its start hba)
pub type BitId = crate::Hba;
// Unique version of BIT (monotonic increased)
pub type BitVersion = u32;
