//! Shadow paging DiskView.
use crate::prelude::*;

// DiskShadow manages IO for HbaRange [start, end) and its backup. When
// user issues an operation on HbaRange [start, end), DiskShadow will
// map the requested `hba` to `hba` or `hba - start + end`, depending
// on the current bitmap.
// Typical layout of DiskShadow:
// start           end     (2 * end - start)
//   +--------------+--------------+---------------+---------------+
//   |    block     | block_shadow |    bitmap     | bitmap_shadow |
//   +--------------+--------------+---------------+---------------+
#[derive(Debug)]
pub struct DiskShadow {
    boundary: HbaRange,
    current_block: BitMap,
    dirty_block: BitMap,
    current_bitmap: bool,
    disk: DiskView,
}

impl DiskShadow {
    pub fn new(boundary: HbaRange, disk: DiskView) -> Self {
        let nr_blocks = boundary.num_covered_blocks();
        let nr_bitmap_blocks = Self::nr_bitmap_blocks(nr_blocks);
        let total_blocks = (nr_blocks + nr_bitmap_blocks) * 2;
        let end_addr = boundary.start() + (total_blocks as _);
        debug_assert!(
            (!boundary.is_empty() && end_addr <= disk.boundary().end()),
            "DiskShadow::new check boundary failed: {:?}",
            boundary
        );

        let current_block = BitMap::repeat(false, nr_blocks);
        let dirty_block = BitMap::repeat(false, nr_blocks);
        let current_bitmap = false;
        Self {
            boundary,
            current_block,
            dirty_block,
            current_bitmap,
            disk,
        }
    }

    pub async fn load(boundary: HbaRange, disk: DiskView, shadow: bool) -> Result<Self> {
        let nr_blocks = boundary.num_covered_blocks();
        let nr_bitmap_blocks = Self::nr_bitmap_blocks(nr_blocks);
        let total_blocks = (nr_blocks + nr_bitmap_blocks) * 2;
        let end_addr = boundary.start() + (total_blocks as _);
        debug_assert!(
            (!boundary.is_empty() && end_addr <= disk.boundary().end()),
            "DiskShadow::load check boundary failed: {:?}",
            boundary
        );

        let mut bitmap_addr = boundary.start() + (2 * nr_blocks) as _;
        if shadow {
            bitmap_addr = bitmap_addr + (nr_bitmap_blocks as _);
        }
        let mut buffer = Vec::<u8>::with_capacity(nr_bitmap_blocks * BLOCK_SIZE);
        buffer.resize(nr_bitmap_blocks * BLOCK_SIZE, 0);
        disk.read(bitmap_addr, buffer.as_mut_slice()).await?;

        let mut current_block = BitMap::from_vec(buffer);
        current_block.resize(nr_blocks, false);

        Ok(Self {
            boundary,
            current_block,
            current_bitmap: shadow,
            dirty_block: BitMap::repeat(false, nr_blocks),
            disk,
        })
    }

    pub fn nr_bitmap_blocks(nr_blocks: usize) -> usize {
        let nr_bytes = (nr_blocks + BITMAP_UNIT - 1) / BITMAP_UNIT;
        (nr_bytes + BLOCK_SIZE - 1) / BLOCK_SIZE
    }

    pub fn total_blocks_with_shadow(nr_blocks: usize) -> usize {
        let nr_bitmap_blocks = Self::nr_bitmap_blocks(nr_blocks);
        (nr_blocks + nr_bitmap_blocks) * 2
    }

    pub fn boundary(&self) -> &HbaRange {
        &self.boundary
    }

    fn check_boundary(&self, hba: Hba) -> Result<()> {
        if !self.boundary.is_within_range(hba) {
            return_errno!(EINVAL, "Illegal Hba in DiskShadow");
        }
        Ok(())
    }

    fn offset(&self, hba: Hba) -> usize {
        (hba.to_raw() - self.boundary.start().to_raw()) as usize
    }

    fn shadow(&self, hba: Hba) -> bool {
        let offset = self.offset(hba);
        self.current_block[offset]
    }

    pub fn mark_dirty(&mut self, hba: Hba) {
        let offset = self.offset(hba);
        if !self.dirty_block[offset] {
            self.dirty_block.set(offset, true);
            let shadow = self.current_block[offset];
            self.current_block.set(offset, !shadow);
        }
    }

    fn block_addr(&self, hba: Hba, shadow: bool) -> Hba {
        if shadow {
            hba - self.boundary.start().to_raw() + self.boundary.end().to_raw()
        } else {
            hba
        }
    }

    fn bitmap_addr(&self, shadow: bool) -> Hba {
        let nr_blocks = self.boundary.num_covered_blocks();
        let start = self.boundary.start() + (2 * nr_blocks) as _;
        let nr_bitmap_blocks = Self::nr_bitmap_blocks(nr_blocks);
        if shadow {
            start + (nr_bitmap_blocks as _)
        } else {
            start
        }
    }

    pub async fn read(&self, hba: Hba, buf: &mut [u8]) -> Result<usize> {
        debug_assert!(buf.len() <= BLOCK_SIZE);
        self.check_boundary(hba)?;

        let shadow = self.shadow(hba);
        self.disk.read(self.block_addr(hba, shadow), buf).await
    }

    pub async fn write(&mut self, hba: Hba, buf: &[u8]) -> Result<usize> {
        debug_assert!(buf.len() <= BLOCK_SIZE);
        self.check_boundary(hba)?;

        let offset = self.offset(hba);
        let mut shadow = self.shadow(hba);
        if !self.dirty_block[offset] {
            shadow = !shadow;
        }
        let result = self.disk.write(self.block_addr(hba, shadow), buf).await?;
        self.mark_dirty(hba);
        Ok(result)
    }

    pub async fn persist(&mut self, checkpoint: bool) -> Result<bool> {
        if checkpoint {
            self.checkpoint().await?;
        } else {
            self.disk.sync().await?;
        }
        Ok(self.current_bitmap)
    }

    async fn checkpoint(&mut self) -> Result<()> {
        self.current_bitmap = !self.current_bitmap;
        let bitmap_addr = self.bitmap_addr(self.current_bitmap);

        let len = self.current_block.as_raw_slice().len();
        let mut buffer = Vec::<u8>::with_capacity(align_up(len, BLOCK_SIZE));
        buffer.resize(align_up(len, BLOCK_SIZE), 0);
        buffer[..len].copy_from_slice(self.current_block.as_raw_slice());

        self.disk.write(bitmap_addr, buffer.as_slice()).await?;
        self.dirty_block.fill(false);
        Ok(())
    }
}

mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[allow(unused)]
    fn disk_shadow_new(boundary: HbaRange) -> DiskShadow {
        let total_blocks = 16 * MiB / BLOCK_SIZE;
        let disk = Arc::new(MemDisk::new(total_blocks).unwrap());
        DiskShadow::new(boundary, DiskView::new_unchecked(disk))
    }

    #[test]
    fn disk_shadow_write_and_read() {
        async_rt::task::block_on(async move {
            let boundary = HbaRange::new(Hba::new(128)..Hba::new(256));
            let mut disk = disk_shadow_new(boundary);

            let hba = Hba::new(130);
            let offset = disk.offset(hba);
            let mut buffer = Vec::<u8>::with_capacity(BLOCK_SIZE);
            buffer.resize(BLOCK_SIZE, 1);

            disk.write(hba, buffer.as_slice()).await;
            assert_eq!(disk.current_block[offset], true);
            assert_eq!(disk.dirty_block[offset], true);

            buffer.fill(0);
            disk.read(hba, buffer.as_mut_slice()).await;
            assert_eq!(buffer, vec![1u8; 4096]);
        });
    }
    #[test]
    fn disk_shadow_checkpoint() {
        async_rt::task::block_on(async move {
            let boundary = HbaRange::new(Hba::new(128)..Hba::new(256));
            let mut disk = disk_shadow_new(boundary);

            let hba = Hba::new(128);
            let offset = disk.offset(hba);
            let mut buffer = Vec::<u8>::with_capacity(BLOCK_SIZE);
            buffer.resize(BLOCK_SIZE, 1);

            disk.write(hba, buffer.as_slice()).await;
            assert_eq!(disk.current_bitmap, false);
            disk.checkpoint().await;
            assert_eq!(disk.current_block[offset], true);
            assert_eq!(disk.dirty_block[offset], false);
            assert_eq!(disk.current_bitmap, true);
        });
    }
    #[test]
    fn disk_shadow_load() {
        async_rt::task::block_on(async move {
            let total_blocks = 16 * MiB / BLOCK_SIZE;
            let disk = Arc::new(MemDisk::new(total_blocks).unwrap());
            let disk_view = DiskView::new_unchecked(disk);
            let boundary = HbaRange::new(Hba::new(0)..Hba::new(256));
            let mut disk_shadow = DiskShadow::new(boundary.clone(), disk_view.clone());

            let hba = Hba::new(21);
            let offset = disk_shadow.offset(hba);
            disk_shadow.mark_dirty(Hba::new(21));
            disk_shadow.checkpoint().await;

            let disk_shadow = DiskShadow::load(boundary.clone(), disk_view.clone(), false)
                .await
                .unwrap();
            assert_eq!(disk_shadow.current_bitmap, false);
            assert_eq!(disk_shadow.current_block[offset], false);
            let disk_shadow = DiskShadow::load(boundary, disk_view, true).await.unwrap();
            assert_eq!(disk_shadow.current_bitmap, true);
            assert_eq!(disk_shadow.current_block[offset], true);
        });
    }
}
