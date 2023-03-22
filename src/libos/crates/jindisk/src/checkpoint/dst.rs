//! Data Segment Table (DST).
use super::BitMap;
use crate::prelude::*;

use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fmt::{self, Debug};

/// Data Segment Table.
/// Manage per-segment metadata of data segments (valid block bitmap).
pub struct DST {
    region_addr: Hba,
    num_segments: usize,
    // K: Data segment start hba
    // V: (A bitmap where each bit indicates whether a block within is valid, Number of valid blocks)
    bitmaps: HashMap<Hba, (BitMap, usize)>,
    // Idx: Number of valid blocks
    // V: Set of data segment start hba
    validity_tracker: [HashSet<Hba>; NUM_BLOCKS_PER_SEGMENT + 1],
    curr_victim: Option<Hba>,
}

impl DST {
    pub fn new(data_region_addr: Hba, num_data_segments: usize) -> Self {
        Self {
            region_addr: data_region_addr,
            num_segments: num_data_segments,
            bitmaps: HashMap::with_capacity(num_data_segments),
            validity_tracker: vec![HashSet::new(); NUM_BLOCKS_PER_SEGMENT + 1]
                .try_into()
                .unwrap(),
            curr_victim: None,
        }
    }

    pub fn update_validity(&mut self, blocks: &[Hba], is_valid: bool) {
        blocks.iter().for_each(|&block_addr| {
            // Get segment addr according to block addr
            let seg_addr = self.calc_seg_addr(block_addr);
            // Get per-segment bitmap
            let (block_bitmap, num_valid) = self.bitmaps.get_mut(&seg_addr).unwrap();

            // Invalid the block, update bitmap and validity counter
            let idx = (block_addr - seg_addr.to_raw()).to_raw() as usize;
            debug_assert!(block_bitmap[idx] != is_valid);
            block_bitmap.set(idx, is_valid);
            self.validity_tracker[*num_valid].remove(&seg_addr);
            *num_valid = {
                if is_valid {
                    *num_valid + 1
                } else {
                    num_valid.saturating_sub(1)
                }
            };
            self.validity_tracker[*num_valid].insert(seg_addr);
        })
    }

    pub fn validate_or_insert(&mut self, segment_addr: Hba) {
        let seg_cap = NUM_BLOCKS_PER_SEGMENT;
        let replaced = self
            .bitmaps
            .insert(segment_addr, (BitMap::repeat(true, seg_cap), seg_cap));
        if let Some((_, num_valid)) = replaced {
            self.validity_tracker[num_valid].remove(&segment_addr);
        }
        self.validity_tracker[seg_cap].insert(segment_addr);

        if self.curr_victim.is_some() && self.curr_victim.unwrap() == segment_addr {
            self.curr_victim.take();
        }
    }

    /// Pick a victim segment.
    pub fn pick_victim(&mut self) -> Option<VictimSegment> {
        // Pick the victim which has most invalid blocks
        for (num_valid, seg_set) in self.validity_tracker.iter().enumerate() {
            if !seg_set.is_empty() {
                for seg_addr in seg_set {
                    let (block_bitmap, valid_cnt) = self.bitmaps.get(seg_addr).unwrap();
                    debug_assert!(num_valid == *valid_cnt);

                    let _ = self.curr_victim.insert(*seg_addr);
                    return Some(VictimSegment::new(
                        *seg_addr,
                        Self::collect_blocks(*seg_addr, block_bitmap, true),
                    ));
                }
            }
        }
        None
    }

    pub fn alloc_blocks(&mut self, total_num: usize) -> Result<Vec<Hba>> {
        let mut block_vec = Vec::with_capacity(total_num);
        let mut updated_segs = vec![];
        let mut updated_blocks = vec![];

        'outer: for seg_set in self.validity_tracker.iter() {
            if seg_set.is_empty() {
                continue;
            }
            for &seg_addr in seg_set.iter() {
                if self.curr_victim.is_some() && self.curr_victim.unwrap() == seg_addr {
                    continue;
                }
                let (block_bitmap, valid_cnt) = self.bitmaps.get(&seg_addr).unwrap();
                if block_vec.len() + (NUM_BLOCKS_PER_SEGMENT - valid_cnt) <= total_num {
                    block_vec.extend_from_slice(&Self::collect_blocks(
                        seg_addr,
                        block_bitmap,
                        false,
                    ));
                    updated_segs.push(seg_addr);
                } else {
                    let invalid_blocks = &Self::collect_blocks(seg_addr, block_bitmap, false)
                        [..total_num - block_vec.len()];
                    block_vec.extend_from_slice(invalid_blocks);
                    updated_blocks.extend_from_slice(invalid_blocks);
                    break 'outer;
                }
            }
        }
        if block_vec.len() < total_num {
            return_errno!(ENOENT, "no free blocks for allocation")
        }
        debug_assert!(block_vec.len() == total_num);

        updated_segs
            .iter()
            .for_each(|seg| self.validate_or_insert(*seg));
        self.update_validity(&updated_blocks, true);
        block_vec.sort();
        Ok(block_vec)
    }

    fn count_num_blocks(block_bitmap: &BitMap, is_valid: bool) -> usize {
        block_bitmap.iter().filter(|bit| *bit == is_valid).count()
    }

    fn collect_blocks(segment_addr: Hba, block_bitmap: &BitMap, is_valid: bool) -> Vec<Hba> {
        let mut invalid_blocks = Vec::new();
        block_bitmap.iter().enumerate().for_each(|(idx, bit)| {
            if *bit == is_valid {
                invalid_blocks.push(segment_addr + idx as _)
            }
        });
        invalid_blocks
    }

    fn from(region_addr: Hba, num_segments: usize, bitmaps: HashMap<Hba, BitMap>) -> Self {
        let mut validity_tracker: [HashSet<Hba>; NUM_BLOCKS_PER_SEGMENT + 1] =
            vec![HashSet::new(); NUM_BLOCKS_PER_SEGMENT + 1]
                .try_into()
                .unwrap();
        let bitmaps = bitmaps
            .into_iter()
            .map(|(seg_addr, block_bitmap)| {
                let num_valid = Self::count_num_blocks(&block_bitmap, true);
                validity_tracker[num_valid].insert(seg_addr);
                (seg_addr, (block_bitmap, num_valid))
            })
            .collect();
        Self {
            region_addr,
            num_segments,
            bitmaps,
            validity_tracker,
            curr_victim: None,
        }
    }

    fn calc_seg_addr(&self, block_addr: Hba) -> Hba {
        Hba::new(align_down(
            (block_addr - self.region_addr.to_raw()).to_raw() as _,
            NUM_BLOCKS_PER_SEGMENT,
        ) as _)
            + self.region_addr.to_raw()
    }

    /// Calculate space cost on disk.
    pub fn calc_size_on_disk(num_data_segments: usize) -> usize {
        let size = BA_SIZE
            + USIZE_SIZE
            + num_data_segments * (BA_SIZE + NUM_BLOCKS_PER_SEGMENT / BITMAP_UNIT)
            + AUTH_ENC_MAC_SIZE
            + USIZE_SIZE;
        align_up(size, BLOCK_SIZE)
    }
}

/// Victim segment.
pub struct VictimSegment {
    segment_addr: Hba,
    valid_blocks: Vec<Hba>,
}

impl VictimSegment {
    pub fn new(segment_addr: Hba, valid_blocks: Vec<Hba>) -> Self {
        Self {
            segment_addr,
            valid_blocks,
        }
    }

    pub fn segment_addr(&self) -> Hba {
        self.segment_addr
    }

    pub fn valid_blocks(&self) -> &Vec<Hba> {
        &self.valid_blocks
    }
}

impl Serialize for DST {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        self.region_addr.encode(encoder)?;
        self.num_segments.encode(encoder)?;
        let bitmaps: HashMap<Hba, BitMap> = self
            .bitmaps
            .iter()
            .map(|(seg_addr, (bitmap, _))| (*seg_addr, bitmap.clone()))
            .collect();
        bitmaps.encode(encoder)
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let mut offset = 0;
        let region_addr = Hba::decode(&buf[offset..offset + BA_SIZE])?;
        offset += BA_SIZE;
        let num_segments = usize::decode(&buf[offset..offset + USIZE_SIZE])?;
        offset += USIZE_SIZE;
        let bitmaps = HashMap::<Hba, BitMap>::decode(&buf[offset..])?;

        Ok(DST::from(region_addr, num_segments, bitmaps))
    }
}

crate::persist_load_checkpoint_region! {DST}

impl Debug for DST {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checkpoint::DST (Data Segment Table)")
            .field("region_addr", &self.region_addr)
            .field("num_segments", &self.num_segments)
            .field("bitmaps_size", &self.bitmaps.len())
            .finish()
    }
}

impl Debug for VictimSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VictimSegment")
            .field("segment_addr", &self.segment_addr)
            .field("num_valid_blocks", &self.valid_blocks.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn test_dst_fns() {
        let mut dst = DST::new(Hba::new(0), 10usize);

        let seg1 = Hba::new(0);
        let seg2 = Hba::from_byte_offset(1 * SEGMENT_SIZE);
        dst.validate_or_insert(seg1);
        dst.validate_or_insert(seg2);

        let invalid_blocks = [seg1, seg1 + 1 as _, seg2];
        dst.update_validity(&invalid_blocks, false);

        let victim = dst.pick_victim().unwrap();
        assert_eq!(victim.segment_addr, seg1);
        assert_eq!(victim.valid_blocks.len(), NUM_BLOCKS_PER_SEGMENT - 2);

        let invalid_blocks = [seg2 + 1 as _, seg2 + 2 as _, seg2 + 5 as _, seg2 + 7 as _];
        dst.update_validity(&invalid_blocks, false);

        let alloc_blocks = dst.alloc_blocks(2).unwrap();
        assert_eq!(alloc_blocks[0], seg2);
        assert_eq!(alloc_blocks[1], seg2 + 1 as _);

        let victim = dst.pick_victim().unwrap();
        assert_eq!(victim.segment_addr, seg2);
        assert_eq!(victim.valid_blocks[1], seg2 + 1 as _);
        assert_eq!(victim.valid_blocks[4], seg2 + 6 as _);
    }

    #[test]
    fn test_dst_serialize() {
        let region_addr = Hba::new(1);
        let mut dst = DST::new(region_addr, 5usize);

        let seg1 = Hba::new(0);
        let seg2 = Hba::from_byte_offset(1 * SEGMENT_SIZE);
        dst.validate_or_insert(seg1);
        dst.validate_or_insert(seg2);

        let mut bytes = Vec::new();
        dst.encode(&mut bytes).unwrap();
        let decoded_dst = DST::decode(&bytes).unwrap();

        assert_eq!(decoded_dst.region_addr, region_addr);
        assert!(decoded_dst.bitmaps.contains_key(&seg1) && decoded_dst.bitmaps.contains_key(&seg2));
        let offset = 42;
        assert_eq!(
            format!("{:?}", dst)[..offset],
            format!("{:?}", decoded_dst)[..offset]
        );
    }

    #[test]
    fn test_dst_persist_load() -> Result<()> {
        async_rt::task::block_on(async move {
            let dst = DST::new(Hba::new(0), 8usize);
            let disk = Arc::new(MemDisk::new(1024usize).unwrap());
            let disk = DiskView::new_unchecked(disk);

            let root_key = DefaultCryptor::gen_random_key();
            dst.persist(&disk, Hba::new(0), &root_key).await?;
            let loaded_dst = DST::load(&disk, Hba::new(0), &root_key).await?;

            assert_eq!(format!("{:?}", dst), format!("{:?}", loaded_dst));
            Ok(())
        })
    }
}
