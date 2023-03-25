//! Reverse Index Table (RIT).
use crate::prelude::*;
use crate::util::DiskShadow;

use std::fmt::{self, Debug};

/// Reverse Index Table.
/// Manage reverse mapping (hba => lba).
pub struct RIT {
    data_region_addr: Hba,
    disk_array: DiskArray<Lba>,
}

const RIT_ENCRYPTED_WITH_HMAC: bool = false;

impl RIT {
    pub fn new(
        data_region_addr: Hba,
        num_data_segments: usize,
        rit_boundary: HbaRange,
        disk: DiskView,
        key: Key,
    ) -> Self {
        Self {
            data_region_addr,
            disk_array: DiskArray::new(
                num_data_segments * NUM_BLOCKS_PER_SEGMENT,
                DiskShadow::new(rit_boundary, disk),
                key,
                RIT_ENCRYPTED_WITH_HMAC,
            ),
        }
    }

    pub async fn insert(&mut self, hba: Hba, lba: Lba) -> Result<()> {
        self.disk_array.set(self.offset(hba), lba).await
    }

    pub async fn find_lba(&mut self, hba: Hba) -> Result<Lba> {
        self.disk_array.get(self.offset(hba)).await
    }

    pub async fn find_and_invalidate(&mut self, hba: Hba) -> Result<Lba> {
        let existed_lba = self.find_lba(hba).await?;
        self.insert(hba, NEGATIVE_LBA).await?;
        Ok(existed_lba)
    }

    pub async fn check_valid(&mut self, hba: Hba, lba: Lba) -> bool {
        self.find_lba(hba).await.unwrap() == lba
    }

    pub fn size(&self) -> usize {
        self.disk_array.table_size()
    }

    fn offset(&self, hba: Hba) -> usize {
        (hba - self.data_region_addr.to_raw()).to_raw() as _
    }

    /// Calculate RIT blocks without shadow block.
    pub fn calc_rit_blocks(num_data_segments: usize) -> usize {
        let nr_units = num_data_segments * NUM_BLOCKS_PER_SEGMENT;
        DiskArray::<Lba>::total_blocks(nr_units)
    }

    /// Calculate space cost in bytes (with shadow blocks) on disk.
    pub fn calc_size_on_disk(num_data_segments: usize) -> usize {
        let nr_units = num_data_segments * NUM_BLOCKS_PER_SEGMENT;
        let total_blocks = DiskArray::<Lba>::total_blocks_with_shadow(nr_units);
        total_blocks * BLOCK_SIZE
    }
}

impl RIT {
    pub async fn persist(&mut self, checkpoint: bool) -> Result<bool> {
        self.disk_array.persist(checkpoint).await
    }

    pub async fn load(
        data_region_addr: Hba,
        num_data_segments: usize,
        rit_boundary: HbaRange,
        disk: DiskView,
        key: Key,
        shadow: bool,
    ) -> Result<Self> {
        let disk_shadow = DiskShadow::load(rit_boundary, disk, shadow).await?;
        Ok(Self {
            data_region_addr,
            disk_array: DiskArray::new(
                num_data_segments * NUM_BLOCKS_PER_SEGMENT,
                disk_shadow,
                key,
                RIT_ENCRYPTED_WITH_HMAC,
            ),
        })
    }
}

impl Debug for RIT {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checkpoint::RIT (Reverse Index Table)")
            .field("table_size", &self.size())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn test_rit_fns() -> Result<()> {
        async_rt::task::block_on(async move {
            let disk_blocks = 64 * MiB / BLOCK_SIZE;
            let disk = Arc::new(MemDisk::new(disk_blocks).unwrap());
            let disk = DiskView::new_unchecked(disk);

            let data_region_addr = Hba::new(0);
            let num_data_segments = 8usize;
            let rit_blocks = RIT::calc_rit_blocks(num_data_segments);
            let rit_blocks_with_shadow = RIT::calc_size_on_disk(num_data_segments) / BLOCK_SIZE;
            // data_blocks: align_up(((8 * 4MiB / 4KiB) * 8B) / (4KiB - 16)) = 17, bitmap_blocks: 1
            assert_eq!(rit_blocks, 17);
            assert_eq!(rit_blocks_with_shadow, 36);

            let rit_start = data_region_addr + (NUM_BLOCKS_PER_SEGMENT * num_data_segments) as _;
            let rit_end = rit_start + (rit_blocks as _);
            let rit_boundary = HbaRange::new(rit_start..rit_end);

            let key = DefaultCryptor::gen_random_key();
            let mut rit = RIT::new(
                data_region_addr,
                num_data_segments,
                rit_boundary.clone(),
                disk.clone(),
                key.clone(),
            );

            let kv1 = (Hba::new(1), Lba::new(2));
            let kv2 = (Hba::new(1025), Lba::new(5));

            rit.insert(kv1.0, kv1.1).await?;
            rit.insert(kv2.0, kv2.1).await?;

            assert_eq!(rit.find_lba(kv1.0).await.unwrap(), kv1.1);
            assert_eq!(rit.find_lba(kv2.0).await.unwrap(), kv2.1);

            assert_eq!(rit.find_and_invalidate(kv2.0).await.unwrap(), kv2.1);
            assert_eq!(rit.check_valid(kv2.0, kv2.1).await, false);

            // Hba is legal only in [0, 8192).
            let kv3 = (Hba::new(8192), Lba::new(0));
            match rit.insert(kv3.0, kv3.1).await {
                Ok(_) => unreachable!(),
                Err(e) => {
                    assert_eq!(e.errno(), EINVAL);
                    assert!(e.to_string().contains("Illegal offset in DiskArray"));
                }
            }

            let shadow = rit.persist(true).await?;

            let mut loaded_rit = RIT::load(
                data_region_addr,
                num_data_segments,
                rit_boundary,
                disk,
                key,
                shadow,
            )
            .await?;
            assert_eq!(loaded_rit.find_lba(kv1.0).await.unwrap(), kv1.1);
            assert_eq!(loaded_rit.check_valid(kv2.0, kv2.1).await, false);
            Ok(())
        })
    }
}
