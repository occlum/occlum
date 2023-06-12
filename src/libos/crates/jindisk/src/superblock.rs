//! Superblock of JinDisk.
use crate::checkpoint::{KeyTable, BITC, DST, RIT, SVT};
use crate::prelude::*;

use std::convert::TryInto;

/// Superblock.
#[derive(Debug, Clone)]
pub struct SuperBlock {
    /// Magic number
    pub magic_number: u32,

    /// Total blocks
    pub total_blocks: usize,

    /// Per block size
    pub block_size: usize,
    /// Per segment size
    pub segment_size: usize,

    /// Number of data segments
    pub num_data_segments: usize,
    /// Number of over provisioning data segments
    pub num_over_provisioning: usize,
    /// Number of index segments
    pub num_index_segments: usize,

    /// Address of the superblock region
    pub superblock_addr: Hba,
    /// Address of the data region
    pub data_region_addr: Hba,
    /// Address of the index region
    pub index_region_addr: Hba,
    /// Addresses and sizes of the checkpoint region
    pub checkpoint_region: CheckpointRegion,
    /// Address of the journal region
    pub journal_region_addr: Hba,
}

/// Sub-region metadata of checkpoint region.
#[derive(Debug, Clone)]
pub struct CheckpointRegion {
    pub region_addr: Hba,

    pub bitc_addr: Hba,
    pub data_svt_addr: Hba,
    pub index_svt_addr: Hba,
    pub dst_addr: Hba,
    pub rit_addr: Hba,
    pub keytable_addr: Hba,

    pub bitc_size: usize,
    pub data_svt_size: usize,
    pub index_svt_size: usize,
    pub dst_size: usize,
    pub rit_size: usize,
    pub keytable_size: usize,

    pub total_size: usize,
}

impl SuperBlock {
    /// Initialize superblock metadata (Calculate disk layout and region size).
    pub fn init(total_blocks: usize) -> Self {
        let total_bytes = total_blocks * BLOCK_SIZE;

        let total_data_bytes = (total_bytes as f32 * DATA_PROPORTION) as usize;
        let num_data_segments = total_data_bytes / SEGMENT_SIZE;
        let num_data_segments = align_up(num_data_segments, BITMAP_UNIT);
        const DATA_OVER_PROVISIONING: usize = 8;
        let num_data_segments = num_data_segments + DATA_OVER_PROVISIONING;

        let num_index_segments = {
            let num_bit = total_data_bytes / DATA_SIZE_PER_BIT + MAX_LEVEL0_BIT_NUM;
            // TODO: Fix this limitation
            align_up(num_bit * 2, BITMAP_UNIT)
        };

        let superblock_addr = SUPER_BLOCK_REGION_ADDR;
        let data_region_addr = superblock_addr
            + Hba::from_byte_offset_aligned(Self::calc_size_on_disk())
                .unwrap()
                .to_raw();
        let index_region_addr =
            data_region_addr + Hba::from_byte_offset(num_data_segments * SEGMENT_SIZE).to_raw();
        let checkpoint_region = CheckpointRegion::from(
            index_region_addr
                + Hba::from_byte_offset(num_index_segments * INDEX_SEGMENT_SIZE).to_raw(),
            num_data_segments,
            num_index_segments,
        );
        let journal_region_addr = checkpoint_region.region_addr
            + Hba::from_byte_offset_aligned(checkpoint_region.total_size)
                .unwrap()
                .to_raw();

        let total_blocks_inuse = journal_region_addr.to_raw() as usize;
        assert!(
            total_blocks_inuse <= total_blocks,
            "[SuperBlock] In-use number of blocks out of limit. In-use: {}, total: {}",
            total_blocks_inuse,
            total_blocks
        );

        Self {
            magic_number: MAGIC_NUMBER,
            total_blocks,
            block_size: BLOCK_SIZE,
            segment_size: SEGMENT_SIZE,
            num_data_segments,
            num_over_provisioning: DATA_OVER_PROVISIONING,
            num_index_segments,
            superblock_addr,
            data_region_addr,
            index_region_addr,
            checkpoint_region,
            journal_region_addr,
        }
    }

    /// Calculate space cost on disk.
    pub fn calc_size_on_disk() -> usize {
        let size = SUPER_BLOCK_SIZE + AUTH_ENC_MAC_SIZE + USIZE_SIZE;
        align_up(size, BLOCK_SIZE)
    }
}

impl CheckpointRegion {
    fn from(region_addr: Hba, num_data_segments: usize, num_index_segments: usize) -> Self {
        let (bitc_size, data_svt_size, index_svt_size, dst_size, rit_size, keytable_size) = (
            BITC::calc_size_on_disk(num_index_segments),
            SVT::calc_size_on_disk(num_data_segments),
            SVT::calc_size_on_disk(num_index_segments),
            DST::calc_size_on_disk(num_data_segments),
            RIT::calc_size_on_disk(num_data_segments),
            KeyTable::calc_size_on_disk(num_data_segments),
        );
        let bitc_addr = region_addr + 1 as _; // PFLAG at begin
        let data_svt_addr = bitc_addr + Hba::from_byte_offset_aligned(bitc_size).unwrap().to_raw();
        let index_svt_addr = data_svt_addr
            + Hba::from_byte_offset_aligned(data_svt_size)
                .unwrap()
                .to_raw();
        let dst_addr = index_svt_addr
            + Hba::from_byte_offset_aligned(index_svt_size)
                .unwrap()
                .to_raw();
        let rit_addr = dst_addr + Hba::from_byte_offset_aligned(dst_size).unwrap().to_raw();
        let keytable_addr = rit_addr + Hba::from_byte_offset_aligned(rit_size).unwrap().to_raw();
        let total_size = keytable_addr.to_offset() + keytable_size - region_addr.to_offset();

        Self {
            region_addr,
            bitc_addr,
            data_svt_addr,
            index_svt_addr,
            dst_addr,
            rit_addr,
            keytable_addr,

            bitc_size,
            data_svt_size,
            index_svt_size,
            dst_size,
            rit_size,
            keytable_size,

            total_size,
        }
    }
}

crate::impl_default_serialize! {SuperBlock, SUPER_BLOCK_SIZE}
crate::persist_load_checkpoint_region! {SuperBlock}

#[cfg(test)]
mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn test_sb_init() {
        let sb = SuperBlock::init(100 * GiB / BLOCK_SIZE);
        println!("{:#?}", sb);
        assert_eq!(sb.num_data_segments, 24328);
        assert_eq!(sb.num_index_segments, 384);
    }

    #[test]
    fn test_sb_persist_load() -> Result<()> {
        async_rt::task::block_on(async move {
            let sb = SuperBlock::init(100 * GiB / BLOCK_SIZE);
            let root_key = DefaultCryptor::gen_random_key();
            let disk = Arc::new(MemDisk::new(1024usize).unwrap());
            let disk = DiskView::new_unchecked(disk);

            sb.persist(&disk, SUPER_BLOCK_REGION_ADDR, &root_key)
                .await?;
            let loaded_sb = SuperBlock::load(&disk, SUPER_BLOCK_REGION_ADDR, &root_key).await?;

            assert_eq!(format!("{:?}", sb), format!("{:?}", loaded_sb));
            Ok(())
        })
    }
}
