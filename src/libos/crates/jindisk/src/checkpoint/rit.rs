//! Reverse Index Table (RIT).
use crate::prelude::*;

use std::fmt::{self, Debug};

/// Reverse Index Table.
/// Manage reverse mapping (hba => lba).
pub struct RIT {
    data_region_addr: Hba,
    disk_array: DiskArray<Lba>,
}

impl RIT {
    pub fn new(region_addr: Hba, data_region_addr: Hba, disk: DiskView, key: &Key) -> Self {
        Self {
            data_region_addr,
            disk_array: DiskArray::new(region_addr, disk.clone(), key),
        }
    }

    pub async fn insert(&mut self, hba: Hba, lba: Lba) -> Result<()> {
        self.disk_array.set(self.offset(hba), lba).await
    }

    pub async fn find_lba(&mut self, hba: Hba) -> Option<Lba> {
        self.disk_array.get(self.offset(hba)).await
    }

    pub async fn find_and_invalidate(&mut self, hba: Hba) -> Result<Lba> {
        let existed_lba = self.find_lba(hba).await.unwrap();
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

    /// Calculate space cost on disk.
    pub fn calc_size_on_disk(num_data_segments: usize) -> usize {
        let size = USIZE_SIZE
            + num_data_segments * NUM_BLOCKS_PER_SEGMENT * BA_SIZE * 2
            + AUTH_ENC_MAC_SIZE
            + USIZE_SIZE;
        align_up(size, BLOCK_SIZE)
    }
}

impl RIT {
    pub async fn persist(&self) -> Result<()> {
        self.disk_array.persist().await
    }

    pub async fn load(
        disk: &DiskView,
        region_addr: Hba,
        data_region_addr: Hba,
        root_key: &Key,
    ) -> Result<Self> {
        Ok(Self::new(
            region_addr,
            data_region_addr,
            disk.clone(),
            root_key,
        ))
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
            let disk = Arc::new(MemDisk::new(1024usize).unwrap());
            let disk = DiskView::new_unchecked(disk);
            let root_key = DefaultCryptor::gen_random_key();
            let mut rit = RIT::new(Hba::new(0), Hba::new(0), disk.clone(), &root_key);

            let kv1 = (Hba::new(1), Lba::new(2));
            let kv2 = (Hba::new(1025), Lba::new(5));

            rit.insert(kv1.0, kv1.1).await?;
            rit.insert(kv2.0, kv2.1).await?;

            assert_eq!(rit.find_lba(kv1.0).await.unwrap(), kv1.1);
            assert_eq!(rit.find_lba(kv2.0).await.unwrap(), kv2.1);

            assert_eq!(rit.find_and_invalidate(kv2.0).await.unwrap(), kv2.1);
            assert_eq!(rit.check_valid(kv2.0, kv2.1).await, false);

            rit.persist().await?;
            let mut loaded_rit = RIT::load(&disk, Hba::new(0), Hba::new(0), &root_key).await?;
            assert_eq!(loaded_rit.find_lba(kv1.0).await.unwrap(), kv1.1);

            Ok(())
        })
    }
}
