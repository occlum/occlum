//! Block Index Table Catalog (BITC).
use crate::index::bit::{Bit, BitId, BitVersion, BIT_SIZE};
use crate::index::LsmLevel;
use crate::prelude::*;

use std::convert::TryInto;
use std::fmt::{self, Debug};

/// Block Index Table Catalog.
/// Manage all BITs in index (lsm tree).
pub struct BITC {
    max_bit_version: BitVersion,
    l0_bit: Option<Bit>,
    l1_bits: Vec<Bit>,
}

impl BITC {
    pub fn new() -> Self {
        Self {
            max_bit_version: 0,
            l0_bit: None,
            l1_bits: Vec::new(),
        }
    }

    /// Assign a version to BIT (monotonic increased)
    pub fn assign_version(&mut self) -> BitVersion {
        self.max_bit_version += 1;
        self.max_bit_version
    }

    pub fn insert_bit(&mut self, bit: Bit, level: LsmLevel) -> Option<Bit> {
        let old_l0_bit = match level {
            0 => {
                let old_bit = self.l0_bit.take();
                let _ = self.l0_bit.insert(bit);
                old_bit
            }
            1 => {
                self.l1_bits.push(bit);
                None
            }
            _ => panic!("illegal lsm level"),
        };
        old_l0_bit
    }

    pub fn max_bit_version(&self) -> BitVersion {
        self.max_bit_version
    }

    pub fn l0_bit(&self) -> Option<Bit> {
        self.l0_bit.as_ref().map(|bit| bit.clone())
    }

    pub fn remove_bit(&mut self, bit_id: BitId, level: LsmLevel) {
        match level {
            0 => {
                if let Some(l0_bit) = &self.l0_bit {
                    debug_assert!(l0_bit.id() == bit_id);
                    let _ = self.l0_bit.take();
                }
            }
            1 => {
                self.l1_bits.drain_filter(|l1_bit| l1_bit.id() == bit_id);
            }
            _ => panic!("illegal lsm level"),
        }
    }

    pub fn find_bit_by_lba(&self, target_lba: Lba, level: LsmLevel) -> Option<&Bit> {
        match level {
            // Find level 0
            0 => {
                if let Some(l0_bit) = &self.l0_bit {
                    if l0_bit.lba_range().is_within_range(target_lba) {
                        return Some(l0_bit);
                    }
                }
            }
            // Find level 1
            1 => {
                for bit in &self.l1_bits {
                    if bit.lba_range().is_within_range(target_lba) {
                        return Some(bit);
                    }
                }
            }
            _ => panic!("illegal lsm level"),
        }
        None
    }

    /// Find all `Bit`s which have overlapped lba range with the target range.
    pub fn find_bit_by_lba_range(&self, target_range: &LbaRange, level: LsmLevel) -> Vec<Bit> {
        match level {
            // Find level 0
            0 => {
                if let Some(l0_bit) = &self.l0_bit {
                    if l0_bit.lba_range().is_overlapped(target_range) {
                        return vec![l0_bit.clone()];
                    }
                }
            }
            // Find level 1
            1 => {
                return self
                    .l1_bits
                    .iter()
                    .filter(|l1_bit| l1_bit.lba_range().is_overlapped(target_range))
                    .map(|bit| bit.clone())
                    .collect();
            }
            _ => panic!("illegal lsm level"),
        }
        vec![]
    }

    /// Initialize all BIT node caches.
    pub async fn init_bit_caches(&self, disk: &DiskView) -> Result<()> {
        if self.l0_bit.is_none() {
            return Ok(());
        }
        self.l0_bit.as_ref().unwrap().init_cache(disk).await?;
        for bit in &self.l1_bits {
            bit.init_cache(disk).await?;
        }
        Ok(())
    }

    pub fn from(max_bit_version: BitVersion, l0_bit: Option<Bit>, l1_bits: Vec<Bit>) -> Self {
        Self {
            max_bit_version,
            l0_bit,
            l1_bits,
        }
    }

    /// Calculate space cost on disk.
    pub fn calc_size_on_disk(num_index_segments: usize) -> usize {
        let size = num_index_segments * BIT_SIZE + AUTH_ENC_MAC_SIZE + USIZE_SIZE;
        align_up(size, BLOCK_SIZE)
    }
}

impl Serialize for BITC {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        encoder.write_bytes(&self.max_bit_version.to_le_bytes())?;
        if self.max_bit_version == 0 {
            return Ok(());
        }
        self.l0_bit.as_ref().unwrap().encode(encoder)?;
        self.l1_bits.len().encode(encoder)?;
        for l1_bit in &self.l1_bits {
            l1_bit.encode(encoder)?;
        }
        Ok(())
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let mut offset = 0;
        let decode_err = EINVAL;
        let max_bit_version = BitVersion::from_le_bytes(
            buf[offset..offset + U32_SIZE]
                .try_into()
                .map_err(|_| decode_err)?,
        );
        offset += U32_SIZE;
        if max_bit_version == 0 {
            return Ok(Self::new());
        }
        let l0_bit = Some(Bit::decode(&buf[offset..offset + BIT_SIZE])?);
        offset += BIT_SIZE;

        let l1_len = usize::decode(&buf[offset..offset + USIZE_SIZE])?;
        offset += USIZE_SIZE;
        let mut l1_bits = Vec::with_capacity(l1_len);
        for _ in 0..l1_len {
            l1_bits.push(Bit::decode(&buf[offset..offset + BIT_SIZE])?);
            offset += BIT_SIZE;
        }

        Ok(BITC::from(max_bit_version, l0_bit, l1_bits))
    }
}

crate::persist_load_checkpoint_region! {BITC}

impl Debug for BITC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checkpoint::BITC (Block Index Table Catalog)")
            .field("max_bit_version", &self.max_bit_version)
            .field("level_0_bit", &self.l0_bit)
            .field("level_1_bits", &self.l1_bits)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn test_bitc_fns() {
        let mut bitc = BITC::new();
        let level0 = 0 as LsmLevel;
        let level1 = 1 as LsmLevel;

        let searched_bit = bitc.find_bit_by_lba(Lba::new(0), level0);
        assert!(searched_bit.is_none());

        let old_l0_bit = bitc.insert_bit(
            Bit::new_unchecked(BitId::new(0), LbaRange::new(Lba::new(0)..Lba::new(9))),
            level0,
        );

        assert!(old_l0_bit.is_none());

        let id = BitId::from_byte_offset(2 * INDEX_SEGMENT_SIZE);
        let old_l0_bit = bitc.insert_bit(
            Bit::new_unchecked(id, LbaRange::new(Lba::new(10)..Lba::new(19))),
            level0,
        );

        assert!(old_l0_bit.is_some());

        let searched_bit = bitc.find_bit_by_lba(Lba::new(15), level0);
        assert!(searched_bit.unwrap().id() == id);

        let id = BitId::from_byte_offset(5 * INDEX_SEGMENT_SIZE);
        let old_l0_bit = bitc.insert_bit(
            Bit::new_unchecked(id, LbaRange::new(Lba::new(20)..Lba::new(29))),
            level1,
        );

        assert!(old_l0_bit.is_none());

        let searched_bit = bitc.find_bit_by_lba(Lba::new(25), level1);
        assert!(searched_bit.unwrap().id() == id);

        let searched_bit =
            bitc.find_bit_by_lba_range(&LbaRange::new(Lba::new(20)..Lba::new(25)), level1);
        assert!(searched_bit[0].id() == id);

        assert!(
            bitc.find_bit_by_lba_range(&LbaRange::new(Lba::new(15)..Lba::new(25)), level1)[0].id()
                == id
        );
        assert!(bitc
            .find_bit_by_lba_range(&LbaRange::new(Lba::new(10)..Lba::new(15)), level1)
            .is_empty());
    }

    #[test]
    fn test_bitc_serialize() {
        let mut bitc = BITC::new();
        bitc.assign_version();
        let id = BitId::from_byte_offset(5 * INDEX_SEGMENT_SIZE);
        bitc.insert_bit(
            Bit::new_unchecked(id, LbaRange::new(Lba::new(0)..Lba::new(9))),
            0 as LsmLevel,
        );
        bitc.insert_bit(
            Bit::new_unchecked(BitId::new(0), LbaRange::new(Lba::new(10)..Lba::new(19))),
            1 as LsmLevel,
        );

        let mut bytes = Vec::new();
        bitc.encode(&mut bytes).unwrap();
        let decoded_bitc = BITC::decode(&bytes).unwrap();

        assert_eq!(decoded_bitc.l0_bit().unwrap().id(), id);
        assert_eq!(format!("{:?}", bitc), format!("{:?}", decoded_bitc));
    }

    #[test]
    fn test_bitc_persist_load() -> Result<()> {
        async_rt::task::block_on(async move {
            let bitc = BITC::new();
            let disk = Arc::new(MemDisk::new(1024usize).unwrap());
            let disk = DiskView::new_unchecked(disk);
            let root_key = DefaultCryptor::gen_random_key();

            bitc.persist(&disk, Hba::new(0), &root_key).await?;
            let loaded_bitc = BITC::load(&disk, Hba::new(0), &root_key).await?;

            assert_eq!(format!("{:?}", bitc), format!("{:?}", loaded_bitc));
            Ok(())
        })
    }
}
