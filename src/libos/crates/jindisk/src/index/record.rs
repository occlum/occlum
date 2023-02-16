//! Record. A cryption metadata for encrypted data block.
use crate::prelude::*;

use std::fmt::{self, Debug};
use std::hash::Hash;
use std::mem::size_of;

// Size of records
pub const RECORD_SIZE: usize = size_of::<Record>();
pub const INTERNAL_RECORD_SIZE: usize = size_of::<InternalRecord>();
pub const ROOT_RECORD_SIZE: usize = size_of::<RootRecord>();
pub const LEAF_RECORD_SIZE: usize = size_of::<LeafRecord>();

/// Record.
/// lba => {hba, cipher_meta {mac, key (deprecated), iv (deprecated)}}
#[repr(C)]
#[derive(Clone, Debug)]
pub struct Record {
    lba: Lba,
    hba: Hba,
    cipher_meta: CipherMeta,
}

/// Root node of BIT.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct RootRecord(InternalRecord);

/// Internal node of BIT.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct InternalRecord {
    // Lba range of pointed block
    lba_range: LbaRange,
    // Hba of pointed block
    hba: Hba,
    cipher_meta: CipherMeta,
}

/// Leaf node of BIT.
#[repr(C)]
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LeafRecord(InternalRecord);

impl Record {
    pub fn new(lba: Lba, hba: Hba, cipher_meta: CipherMeta) -> Self {
        Self {
            lba,
            hba,
            cipher_meta,
        }
    }

    pub fn lba(&self) -> Lba {
        self.lba
    }

    pub fn hba(&self) -> Hba {
        self.hba
    }

    pub fn cipher_meta(&self) -> &CipherMeta {
        &self.cipher_meta
    }

    pub fn new_negative(lba: Lba) -> Self {
        Self {
            lba,
            hba: NEGATIVE_HBA,
            cipher_meta: CipherMeta::new_uninit(),
        }
    }

    pub fn is_negative(&self) -> bool {
        self.hba == NEGATIVE_HBA
    }

    pub fn padding_records(records: &mut Vec<Record>, align: usize) {
        if records.is_empty() || records.len() == align {
            return;
        }
        let last_one = records.last().unwrap().clone();
        records.resize_with(align_up(records.len(), align), || last_one.clone());
    }

    // Test-purpose
    #[allow(unused)]
    pub fn new_uninit() -> Self {
        Self {
            lba: Lba::new(0),
            hba: Hba::new(0),
            cipher_meta: CipherMeta::new_uninit(),
        }
    }
}

impl RootRecord {
    pub fn new(lba_range: LbaRange, hba: Hba, cipher_meta: CipherMeta) -> Self {
        Self(InternalRecord::new(lba_range, hba, cipher_meta))
    }

    pub fn lba_range(&self) -> &LbaRange {
        &self.0.lba_range
    }

    pub fn hba(&self) -> Hba {
        self.0.hba
    }

    pub fn cipher_meta(&self) -> &CipherMeta {
        &self.0.cipher_meta
    }
}

impl InternalRecord {
    pub fn new(lba_range: LbaRange, hba: Hba, cipher_meta: CipherMeta) -> Self {
        Self {
            lba_range,
            hba,
            cipher_meta,
        }
    }

    pub fn lba_range(&self) -> &LbaRange {
        &self.lba_range
    }

    pub fn hba(&self) -> Hba {
        self.hba
    }

    pub fn cipher_meta(&self) -> &CipherMeta {
        &self.cipher_meta
    }
}

impl LeafRecord {
    pub fn new(lba_range: LbaRange, hba: Hba, cipher_meta: CipherMeta) -> Self {
        Self(InternalRecord::new(lba_range, hba, cipher_meta))
    }

    pub fn lba_range(&self) -> &LbaRange {
        &self.0.lba_range()
    }

    pub fn hba(&self) -> Hba {
        self.0.hba()
    }

    pub fn cipher_meta(&self) -> &CipherMeta {
        &self.0.cipher_meta()
    }
}

impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.lba == other.lba && self.hba == other.hba
    }
}

impl Eq for Record {}

impl Hash for InternalRecord {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.lba_range.hash(state);
        self.hba.hash(state);
    }
}

impl PartialEq for InternalRecord {
    fn eq(&self, other: &Self) -> bool {
        self.lba_range == other.lba_range && self.hba == other.hba
    }
}

impl Eq for InternalRecord {}

struct RecordConfig;
impl Debug for RecordConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordConfig")
            .field("RECORD_SIZE", &RECORD_SIZE)
            .field("INTERNAL_RECORD_SIZE", &INTERNAL_RECORD_SIZE)
            .field("ROOT_RECORD_SIZE", &ROOT_RECORD_SIZE)
            .field("LEAF_RECORD_SIZE", &LEAF_RECORD_SIZE)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_record_config() {
        println!("{:#?}", RecordConfig);
    }

    #[test]
    fn test_record_fns() {
        let mut records = vec![
            Record::new(Lba::new(0), Hba::new(0), CipherMeta::new_uninit()),
            Record::new(Lba::new(1), Hba::new(15), CipherMeta::new_uninit()),
            Record::new_negative(Lba::new(5)),
        ];
        let align = 8usize;
        Record::padding_records(&mut records, align);
        assert!(records.len() == align && records.last().unwrap().is_negative());
    }

    #[test]
    fn records_binary_search() -> Result<()> {
        let records = vec![
            InternalRecord::new(
                LbaRange::new(Lba::new(0)..Lba::new(4)),
                Hba::new(0),
                CipherMeta::new_uninit(),
            ),
            InternalRecord::new(
                LbaRange::new(Lba::new(5)..Lba::new(9)),
                Hba::new(0),
                CipherMeta::new_uninit(),
            ),
            InternalRecord::new(
                LbaRange::new(Lba::new(10)..Lba::new(14)),
                Hba::new(0),
                CipherMeta::new_uninit(),
            ),
        ];

        let target_lba = Lba::new(7);

        let cmp_fn = |record: &InternalRecord| {
            if record.lba_range().end() <= target_lba {
                std::cmp::Ordering::Less
            } else if record.lba_range.is_within_range(target_lba) {
                std::cmp::Ordering::Equal
            } else {
                std::cmp::Ordering::Greater
            }
        };

        assert_eq!(records.binary_search_by(cmp_fn).unwrap(), 1);
        Ok(())
    }
}
