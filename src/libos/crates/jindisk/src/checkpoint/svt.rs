//! Segment Validity Table (SVT).
use super::BitMap;
use crate::prelude::*;
use errno::return_errno;

use std::convert::TryInto;
use std::fmt::{self, Debug};

/// Segment Validity Table.
/// Manage allocation/deallocation of data/index segments.
pub struct SVT {
    region_addr: Hba,
    num_segments: usize,
    segment_size: usize,
    num_allocated: usize,
    // A bitmap where each bit indicates whether a segment is valid
    bitmap: BitMap,
}

impl SVT {
    pub fn new(region_addr: Hba, num_segments: usize, segment_size: usize) -> Self {
        Self {
            region_addr,
            num_segments,
            segment_size,
            num_allocated: 0,
            bitmap: BitMap::repeat(true, num_segments),
        }
    }

    pub fn pick_avail_seg(&mut self) -> Result<Hba> {
        let avail_seg = self.find_avail_seg()?;
        self.invalidate_seg(avail_seg);

        self.num_allocated = self.num_allocated.saturating_add(1);
        Ok(avail_seg)
    }

    pub fn validate_seg(&mut self, seg_addr: Hba) {
        let idx = self.calc_bitmap_idx(seg_addr);
        self.bitmap.set(idx, true);

        self.num_allocated = self.num_allocated.saturating_sub(1);
    }

    pub fn num_segments(&self) -> usize {
        self.num_segments
    }

    pub fn num_allocated(&self) -> usize {
        self.num_allocated
    }

    fn find_avail_seg(&self) -> Result<Hba> {
        for (idx, bit) in self.bitmap.iter().enumerate() {
            if *bit {
                return Ok(Hba::from_byte_offset_aligned(idx * self.segment_size)?
                    + self.region_addr.to_raw());
            }
        }

        return_errno!(ENOMEM, "no available memory for segment");
    }

    fn invalidate_seg(&mut self, seg_addr: Hba) {
        let idx = self.calc_bitmap_idx(seg_addr);
        self.bitmap.set(idx, false);
    }

    fn from(
        region_addr: Hba,
        num_segments: usize,
        segment_size: usize,
        num_allocated: usize,
        bitmap: BitMap,
    ) -> Self {
        Self {
            region_addr,
            num_segments,
            segment_size,
            num_allocated,
            bitmap,
        }
    }

    fn calc_bitmap_idx(&self, seg_addr: Hba) -> usize {
        debug_assert!((seg_addr - self.region_addr.to_raw()).to_offset() % self.segment_size == 0);

        (seg_addr - self.region_addr.to_raw()).to_offset() / self.segment_size
    }

    /// Calculate space cost on disk.
    pub fn calc_size_on_disk(num_segments: usize) -> usize {
        let size =
            BA_SIZE + USIZE_SIZE * 3 + num_segments / BITMAP_UNIT + AUTH_ENC_MAC_SIZE + USIZE_SIZE;
        align_up(size, BLOCK_SIZE)
    }
}

impl Serialize for SVT {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        self.region_addr.encode(encoder)?;
        self.num_segments.encode(encoder)?;
        self.segment_size.encode(encoder)?;
        self.num_allocated.encode(encoder)?;
        self.bitmap.encode(encoder)
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
        let segment_size = usize::decode(&buf[offset..offset + USIZE_SIZE])?;
        offset += USIZE_SIZE;
        let num_allocated = usize::decode(&buf[offset..offset + USIZE_SIZE])?;
        offset += USIZE_SIZE;
        let bitmap = BitMap::decode(&buf[offset..])?;

        Ok(SVT::from(
            region_addr,
            num_segments,
            segment_size,
            num_allocated,
            bitmap,
        ))
    }
}

crate::persist_load_checkpoint_region! {SVT}

impl Debug for SVT {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checkpoint::SVT (Segment Validity Table)")
            .field("region_addr", &self.region_addr)
            .field("num_segments", &self.num_segments)
            .field("segment_size", &self.segment_size)
            .field("num_allocated", &self.num_allocated)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn test_svt_fns() {
        let data_region_addr = Hba::new(0);
        let mut data_svt = SVT::new(data_region_addr, 8usize, SEGMENT_SIZE);
        assert_eq!(data_svt.pick_avail_seg().unwrap(), data_region_addr);
        assert_eq!(
            data_svt.pick_avail_seg().unwrap(),
            data_region_addr + Hba::from_byte_offset(1 * SEGMENT_SIZE).to_raw()
        );

        let index_region_addr = Hba::from_byte_offset(8 * SEGMENT_SIZE);
        let index_seg_hba = Hba::from_byte_offset(1 * INDEX_SEGMENT_SIZE);
        let mut index_svt = SVT::new(index_region_addr, 2usize, INDEX_SEGMENT_SIZE);
        assert_eq!(index_svt.pick_avail_seg().unwrap(), index_region_addr);
        assert_eq!(
            index_svt.pick_avail_seg().unwrap(),
            index_region_addr + index_seg_hba.to_raw()
        );
        index_svt.validate_seg(index_region_addr + index_seg_hba.to_raw());
        assert_eq!(
            index_svt.pick_avail_seg().unwrap(),
            index_region_addr + index_seg_hba.to_raw()
        );
    }

    #[test]
    fn test_svt_serialize() {
        let num_segments = 8usize;
        let mut svt = SVT::new(Hba::new(1), 8usize, SEGMENT_SIZE);
        for _ in 0..num_segments / 2 {
            svt.pick_avail_seg().unwrap();
        }

        let mut bytes = Vec::new();
        svt.encode(&mut bytes).unwrap();
        let decoded_svt = SVT::decode(&bytes).unwrap();

        let offset = 82;
        assert_eq!(
            format!("{:?}", svt)[..offset],
            format!("{:?}", decoded_svt)[..offset]
        );
    }

    #[test]
    fn test_svt_persist_load() -> Result<()> {
        async_rt::task::block_on(async move {
            let svt = SVT::new(Hba::new(0), 8usize, SEGMENT_SIZE);
            let disk = Arc::new(MemDisk::new(1024usize).unwrap());
            let disk = DiskView::new_unchecked(disk);
            let root_key = DefaultCryptor::gen_random_key();

            svt.persist(&disk, Hba::new(0), &root_key).await?;
            let loaded_svt = SVT::load(&disk, Hba::new(0), &root_key).await?;

            let offset = 82;
            assert_eq!(
                format!("{:?}", svt)[..offset],
                format!("{:?}", loaded_svt)[..offset]
            );
            Ok(())
        })
    }
}
