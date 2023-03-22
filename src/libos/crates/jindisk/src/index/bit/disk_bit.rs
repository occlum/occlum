//! Block Index Table (BIT).
//!
//! A B+ tree structure integrated with MHT.
//! Responsible for doing disk management, query processing, and security protection.
//!
//! **BIT architecture:**
//! ```text
//!               DiskBit {RootRecord}             ---------
//!               manages  |                        Level 0
//!                        v
//!                     RootBlock                  ---------
//!                  /               \
//!                 /                 \
//!          InternalRecord     InternalRecord     ---------
//!           manages |          manages |          Level 1
//!                   v                  v
//!            InternalBlock       InternalBlock   ---------
//!              /           \
//!             /             \
//!        LeafRecord        LeafRecord            ---------
//!               | manages
//!               v
//!            LeafBlock                            Level 2
//!            /         \
//!           /           \
//!        Record       Record                     ---------
//! ```
//!
//! **BIT on-disk view:**
//! ```text
//! |    RootBlock     |     InternalBlock      |      LeafBlock       |
//! | ROOT_REGION_SIZE |  INTERNAL_REGION_SIZE  |   LEAF_REGION_SIZE   |
//! |                         BIT_SIZE_ON_DISK                         |
//! ```
use super::super::record::{InternalRecord, LeafRecord, Record, RootRecord, RECORD_SIZE};
use super::super::LsmLevel;
use super::{BitId, BitVersion};
use crate::prelude::*;

use std::convert::TryInto;
use std::fmt::{self, Debug};
use std::mem::size_of;

// Size of nodes(blocks) of BIT
pub const BIT_SIZE: usize = size_of::<DiskBit>();
pub const ROOT_BLOCK_SIZE: usize = size_of::<RootBlock>();
pub const INTERNAL_BLOCK_SIZE: usize = size_of::<InternalBlock>();
pub const LEAF_BLOCK_SIZE: usize = size_of::<LeafBlock>();

// Amount of records within nodes(blocks) of BIT
pub const MAX_INTERNAL_RECORD_NUM_PER_ROOT: usize = 32; // BLOCK_SIZE / INTERNAL_RECORD_SIZE
pub const MAX_LEAF_RECORD_NUM_PER_INTERNAL: usize = 32; // BLOCK_SIZE / LEAF_RECORD_SIZE
pub const MAX_LEAF_RECORD_NUM_PER_BIT: usize =
    MAX_INTERNAL_RECORD_NUM_PER_ROOT * MAX_LEAF_RECORD_NUM_PER_INTERNAL;
pub const MAX_RECORD_NUM_PER_LEAF: usize = BLOCK_SIZE / RECORD_SIZE;
pub const MAX_RECORD_NUM_PER_BIT: usize = MAX_LEAF_RECORD_NUM_PER_BIT * MAX_RECORD_NUM_PER_LEAF;

// Region offset and size within an on-disk BIT
// Align with 4KB block
pub const ROOT_REGION_OFFSET: usize = 0;
pub const ROOT_REGION_SIZE: usize = align_up(ROOT_BLOCK_SIZE, BLOCK_SIZE);
pub const INTERNAL_REGION_OFFSET: usize = ROOT_REGION_OFFSET + ROOT_REGION_SIZE;
pub const INTERNAL_REGION_SIZE: usize =
    MAX_INTERNAL_RECORD_NUM_PER_ROOT * align_up(INTERNAL_BLOCK_SIZE, BLOCK_SIZE);
pub const LEAF_REGION_OFFSET: usize = INTERNAL_REGION_OFFSET + INTERNAL_REGION_SIZE;
pub const LEAF_REGION_SIZE: usize = MAX_LEAF_RECORD_NUM_PER_BIT * LEAF_BLOCK_SIZE;

// On-disk BIT size
pub const BIT_SIZE_ON_DISK: usize = ROOT_REGION_SIZE + INTERNAL_REGION_SIZE + LEAF_REGION_SIZE;

#[repr(C)]
#[derive(Clone, Debug)]
/// Block Index Table (on-disk).
pub struct DiskBit {
    /// Unique Id of BIT
    /// This Id equals to the start hba of the BIT
    id: BitId,

    /// Unique version of BIT, larger version refers to more recent one
    version: BitVersion,

    /// Lsm level of BIT
    level: LsmLevel,

    /// Root record node (point to the root block node)
    root_record: RootRecord,

    /// Cryption key for the BIT
    key: Key,
}

#[repr(C)]
#[derive(Clone, Debug)]
/// On-disk unit of root node
pub struct RootBlock {
    /// children: internal record array
    internal_records: [InternalRecord; MAX_INTERNAL_RECORD_NUM_PER_ROOT],
}

#[repr(C)]
#[derive(Clone, Debug)]
/// On-disk unit of indirect node
pub struct InternalBlock {
    /// children: leaf record array
    leaf_records: [LeafRecord; MAX_LEAF_RECORD_NUM_PER_INTERNAL],
}

#[repr(C)]
#[derive(Clone, Debug)]
/// On-disk unit of leaf node
pub struct LeafBlock {
    /// children: record array
    records: [Record; MAX_RECORD_NUM_PER_LEAF],
}

impl DiskBit {
    pub fn new(
        id: BitId,
        version: BitVersion,
        level: LsmLevel,
        root_record: RootRecord,
        key: Key,
    ) -> Self {
        Self {
            id,
            version,
            level,
            root_record,
            key,
        }
    }

    pub fn id(&self) -> BitId {
        self.id
    }

    pub fn lba_range(&self) -> &LbaRange {
        self.root_record.lba_range()
    }

    pub fn version(&self) -> BitVersion {
        self.version
    }

    pub fn level(&self) -> LsmLevel {
        self.level
    }

    pub fn root_record(&self) -> &RootRecord {
        &self.root_record
    }

    pub fn key(&self) -> &Key {
        &self.key
    }

    // Test-purpose
    #[allow(unused)]
    pub(crate) fn new_unchecked(id: BitId, lba_range: LbaRange) -> Self {
        Self {
            id,
            version: 0,
            level: 0,
            root_record: RootRecord::new(lba_range, Hba::new(0), CipherMeta::new_uninit()),
            key: DefaultCryptor::gen_random_key(),
        }
    }
}

impl RootBlock {
    pub fn new(internal_records: [InternalRecord; MAX_INTERNAL_RECORD_NUM_PER_ROOT]) -> Self {
        Self { internal_records }
    }

    pub fn internal_records(&self) -> &[InternalRecord; MAX_INTERNAL_RECORD_NUM_PER_ROOT] {
        &self.internal_records
    }
}

impl InternalBlock {
    pub fn new(leaf_records: [LeafRecord; MAX_LEAF_RECORD_NUM_PER_INTERNAL]) -> Self {
        Self { leaf_records }
    }

    pub fn leaf_records(&self) -> &[LeafRecord; MAX_LEAF_RECORD_NUM_PER_INTERNAL] {
        &self.leaf_records
    }
}

impl LeafBlock {
    pub fn new(records: [Record; MAX_RECORD_NUM_PER_LEAF]) -> Self {
        Self { records }
    }

    pub fn records(&self) -> &[Record; MAX_RECORD_NUM_PER_LEAF] {
        &self.records
    }
}

crate::impl_default_serialize! {DiskBit, BIT_SIZE}
crate::impl_default_serialize! {RootBlock, ROOT_BLOCK_SIZE}
crate::impl_default_serialize! {InternalBlock, INTERNAL_BLOCK_SIZE}
crate::impl_default_serialize! {LeafBlock, LEAF_BLOCK_SIZE}

impl Encoder for [u8; ROOT_BLOCK_SIZE] {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        debug_assert!(self.len() == buf.len());
        self.copy_from_slice(buf);
        Ok(())
    }
}

impl Encoder for [u8; LEAF_BLOCK_SIZE] {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        debug_assert!(self.len() == buf.len());
        self.copy_from_slice(buf);
        Ok(())
    }
}

struct BitConfig;
impl Debug for BitConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BitConfig")
            .field("ROOT_BLOCK_SIZE", &ROOT_BLOCK_SIZE)
            .field("INTERNAL_BLOCK_SIZE", &INTERNAL_BLOCK_SIZE)
            .field("LEAF_BLOCK_SIZE", &LEAF_BLOCK_SIZE)
            .field("ROOT_REGION_SIZE", &ROOT_REGION_SIZE)
            .field("INTERNAL_REGION_SIZE", &INTERNAL_REGION_SIZE)
            .field("LEAF_REGION_SIZE", &LEAF_REGION_SIZE)
            .field("WHOLE_REGION_SIZE", &BIT_SIZE_ON_DISK)
            .field(
                "MAX_INTERNAL_RECORD_NUM_PER_ROOT",
                &MAX_INTERNAL_RECORD_NUM_PER_ROOT,
            )
            .field(
                "MAX_LEAF_RECORD_NUM_PER_INTERNAL",
                &MAX_LEAF_RECORD_NUM_PER_INTERNAL,
            )
            .field("MAX_LEAF_RECORD_NUM_PER_BIT", &MAX_LEAF_RECORD_NUM_PER_BIT)
            .field("MAX_RECORD_NUM_PER_LEAF", &MAX_RECORD_NUM_PER_LEAF)
            .field("MAX_RECORD_NUM_PER_BIT", &MAX_RECORD_NUM_PER_BIT)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_bit_config() {
        assert_eq!(LEAF_BLOCK_SIZE, BLOCK_SIZE);
        println!("{:#?}", BitConfig);
    }
}
