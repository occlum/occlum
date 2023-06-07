//! JinDisk. A log-structured secure block device for TEEs.
//!
//! **Architecture:**
//! ```text
//! +---------------------------------------------------------------+
//! |                      Checkpoint Region                        |
//! |   +-------------------------------------------------------+   |
//! |   | +------------+    +----+ +---+ +---+ +---+ +--------+ |   |
//! |   | | Superblock |--> |BITC| |SVT| |DST| |RIT| |Keytable| |   |
//! |   | +------------+    +----+ +---+ +---+ +---+ +--------+ |   |
//! |   +-------------------------------------------------------+   |
//! |           ^            |                                      |
//! |           |            |   +------------------------+         |
//! |       +--------+       +-> |  +--------+            |         |
//! |       |Root key|           |  |Memtable|  Memory    |         |
//! |       +--------+           |  +--------+            |         |
//! |           |                |    +-----+             |         |
//! |           v                | L0 | BIT |    Disk     |         |
//! |       +----------------+   |    +-----+             |         |
//! |       |  +--+    +--+  |   |    +-----+ +-----+     |         |
//! |       |  |R |<---| R|  |   | L1 | BIT | | BIT | ... |         |
//! |       |  +--+    +--+  |   |    +-----+ +-----+     |         |
//! |       +----------------+   +------------------------+         |
//! |        Journal Region |        |   Index Region               |
//! |                       v        v                              |
//! |   +-------------------------------------------------------+   |
//! |   | +-----------+   +-----------+   +-----------+         |   |
//! |   | |  Segment  |   |  Segment  |   |B| | | | | |   ...   |   |
//! |   | +-----------+   +-----------+   +-----------+         |   |
//! |   +-------------------------------------------------------+   |
//! |                          Data Region                          |
//! +---------------------------------------------------------------+
//! ```
//!
//! **On-disk view:**
//! ```text
//! -------------------------------------------------------------------------------------
//! | Superblock |   Data region   | Index region | Checkpoint region | Journal region |
//! -------------------------------------------------------------------------------------
//! ```
use crate::prelude::*;
use crate::util::RangeQueryCtx;
use crate::{Checkpoint, Cleaner, DataCache, LsmTree, Record, SuperBlock};
use errno::return_errno;

/// JinDisk.
#[derive(Clone)]
pub struct JinDisk {
    disk: Arc<dyn BlockDevice>,
    superblock: SuperBlock,
    data_cache: Arc<DataCache>,
    lsm_tree: Arc<LsmTree>,
    checkpoint: Arc<Checkpoint>,
    cleaner: Cleaner,
    root_key: Key,
}

impl JinDisk {
    /// Create a new `JinDisk`, initialize basic structures.
    pub fn create(disk: Arc<dyn BlockDevice>, root_key: Key) -> Self {
        let superblock = SuperBlock::init(disk.total_blocks());

        let checkpoint_disk_view = Self::checkpoint_disk_view(&superblock, &disk);
        let checkpoint = Arc::new(Checkpoint::new(
            &superblock,
            checkpoint_disk_view,
            &root_key,
        ));

        let index_disk_view = Self::index_disk_view(&superblock, &disk);
        let lsm_tree = Arc::new(LsmTree::new(index_disk_view, checkpoint.clone()));

        let data_disk_view = Self::data_disk_view(&superblock, &disk);
        let data_cache = Arc::new(DataCache::new(
            BUFFER_POOL_CAPACITY,
            data_disk_view.clone(),
            checkpoint.clone(),
            lsm_tree.clone(),
        ));

        let cleaner = Cleaner::new(
            data_disk_view,
            checkpoint.clone(),
            lsm_tree.clone(),
            data_cache.clone(),
        );
        info!("[JinDisk] successfully created\n {:#?}", superblock);

        Self {
            disk,
            superblock,
            data_cache,
            lsm_tree,
            checkpoint,
            cleaner,
            root_key,
        }
    }

    /// Open a created `JinDisk` given the root key.
    ///
    /// * `root_key` - The root key that encrypts basic structures of JinDisk (e.g., SuperBlock and Checkpoint).
    pub async fn open(disk: Arc<dyn BlockDevice>, root_key: Key) -> Result<Self> {
        let superblock_disk_view = Self::superblock_disk_view(&disk);
        let superblock =
            SuperBlock::load(&superblock_disk_view, SUPER_BLOCK_REGION_ADDR, &root_key).await?;
        if superblock.magic_number != MAGIC_NUMBER {
            error!("[JinDisk] open failed");
            return_errno!(EINVAL, "jindisk open error");
        }

        let checkpoint_disk_view = Self::checkpoint_disk_view(&superblock, &disk);
        let checkpoint =
            Arc::new(Checkpoint::load(&checkpoint_disk_view, &superblock, &root_key).await?);

        let index_disk_view = Self::index_disk_view(&superblock, &disk);
        let lsm_tree = Arc::new(LsmTree::new(index_disk_view, checkpoint.clone()));

        let data_disk_view = Self::data_disk_view(&superblock, &disk);
        let data_cache = Arc::new(DataCache::new(
            BUFFER_POOL_CAPACITY,
            data_disk_view.clone(),
            checkpoint.clone(),
            lsm_tree.clone(),
        ));

        let cleaner = Cleaner::new(
            data_disk_view,
            checkpoint.clone(),
            lsm_tree.clone(),
            data_cache.clone(),
        );
        info!(
            "[JinDisk] successfully opened\n{:#?}\n{:#?}",
            superblock, checkpoint
        );

        Ok(Self {
            disk,
            superblock,
            data_cache,
            lsm_tree,
            checkpoint,
            cleaner,
            root_key,
        })
    }

    /// Return superblock of `JinDisk`.
    pub fn superblock(&self) -> &SuperBlock {
        &self.superblock
    }

    /// Return underlying disk of `JinDisk`.
    pub fn disk(&self) -> &Arc<dyn BlockDevice> {
        &self.disk
    }

    /// Return root cryption key of `JinDisk`.
    pub fn root_key(&self) -> &Key {
        &self.root_key
    }

    /// Return upper limit number of blocks that `JinDisk` would occupy.
    pub fn total_blocks(&self) -> usize {
        self.superblock.total_blocks
    }

    /// Return upper limit number of data blocks that `JinDisk` can manage.
    pub fn data_blocks(&self) -> usize {
        (self.superblock.num_data_segments - self.superblock.num_over_provisioning)
            * NUM_BLOCKS_PER_SEGMENT
    }

    /// Return checkpoint of `JinDisk`. (Test-purpose)
    #[allow(unused)]
    pub(crate) fn checkpoint(&self) -> &Arc<Checkpoint> {
        &self.checkpoint
    }

    fn superblock_disk_view(disk: &Arc<dyn BlockDevice>) -> DiskView {
        DiskView::new(
            HbaRange::new(
                SUPER_BLOCK_REGION_ADDR
                    ..(SUPER_BLOCK_REGION_ADDR
                        + Hba::from_byte_offset_aligned(SuperBlock::calc_size_on_disk())
                            .unwrap()
                            .to_raw()),
            ),
            disk.clone(),
        )
    }

    fn data_disk_view(superblock: &SuperBlock, disk: &Arc<dyn BlockDevice>) -> DiskView {
        DiskView::new(
            HbaRange::new(superblock.data_region_addr..superblock.index_region_addr),
            disk.clone(),
        )
    }

    fn index_disk_view(superblock: &SuperBlock, disk: &Arc<dyn BlockDevice>) -> DiskView {
        DiskView::new(
            HbaRange::new(superblock.index_region_addr..superblock.checkpoint_region.region_addr),
            disk.clone(),
        )
    }

    fn checkpoint_disk_view(superblock: &SuperBlock, disk: &Arc<dyn BlockDevice>) -> DiskView {
        DiskView::new(
            HbaRange::new(superblock.index_region_addr..superblock.journal_region_addr),
            disk.clone(),
        )
    }
}

impl JinDisk {
    /// Read a specified number of bytes at a byte offset on the device.
    pub async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.check_rw_args(offset, buf)?;

        // Batch read
        if buf.len() >= BATCH_READ_THRESHOLD {
            let mut query_ctx = RangeQueryCtx::build_from(offset, buf);
            return self
                .read_multi_blocks(&mut query_ctx, buf)
                .await
                .map(|_| buf.len());
        }

        let block_range_iter = BlockRangeIter {
            begin: offset,
            end: offset + buf.len(),
            block_size: BLOCK_SIZE,
        };

        // One-by-one read
        let mut read_len = 0;
        for range in block_range_iter {
            let read_buf = &mut buf[read_len..read_len + range.len()];
            read_len += self.read_one_block(range.block_id, read_buf).await?;
        }

        debug_assert!(read_len == buf.len());
        Ok(read_len)
    }

    /// Write a specified number of bytes at a byte offset on the device.
    pub async fn write(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        self.check_rw_args(offset, buf)?;

        let block_range_iter = BlockRangeIter {
            begin: offset,
            end: offset + buf.len(),
            block_size: BLOCK_SIZE,
        };

        let mut write_len = 0;
        // One-by-one write
        for range in block_range_iter {
            let write_buf = &buf[write_len..write_len + range.len()];
            write_len += self.write_one_block(range.block_id, write_buf).await?;
        }
        debug_assert!(write_len == buf.len());

        // No need for foreground cleaning thanks to threaded logging

        Ok(write_len)
    }

    /// Sync all cached data in the device to the storage medium for durability.
    pub async fn sync(&self) -> Result<()> {
        info!("[JinDisk] sync");
        self.data_cache.persist().await?;

        self.lsm_tree.persist().await?;

        self.checkpoint
            .persist(&self.superblock, &self.root_key)
            .await?;

        let disk_superblock_view = Self::superblock_disk_view(&self.disk);
        self.superblock
            .persist(
                &disk_superblock_view,
                SUPER_BLOCK_REGION_ADDR,
                &self.root_key,
            )
            .await?;

        self.disk.sync().await
    }

    /// Read a single block into the given `buf`.
    async fn read_one_block(&self, bid: Bid, buf: &mut [u8]) -> Result<usize> {
        let target_lba = bid as Lba;

        // Search data segment buffer
        if let Ok(read_len) = self.data_cache.search(target_lba, buf).await {
            return Ok(read_len);
        }

        // Search lsm tree
        if let Some(record) = self.lsm_tree.search(target_lba).await {
            let record = if record.is_negative() {
                // Handle negative record
                self.cleaner.find_migrant(target_lba).unwrap()
            } else {
                record
            };
            self.disk.read(record.hba().to_offset(), buf).await?;
            let decrypted = DefaultCryptor::decrypt_block_aead(
                buf,
                &self.checkpoint.key_table().fetch_key(record.hba()),
                record.cipher_meta(),
            )?;
            buf.copy_from_slice(&decrypted);
        } else {
            // error!("[JinDisk] Read nothing! Target lba: {:?}", target_lba); // Allow empty read
        }

        Ok(buf.len())
    }

    /// Read multi blocks in batches into the given `buf`. Return success
    /// only if all blocks are successfully fetched.
    pub async fn read_multi_blocks(
        &self,
        query_ctx: &mut RangeQueryCtx,
        buf: &mut [u8],
    ) -> Result<()> {
        // Search data segment buffer
        self.data_cache.search_range(query_ctx, buf).await;
        if query_ctx.is_completed() {
            return Ok(());
        }

        // Search lsm tree
        let searched_records = self.lsm_tree.search_range(query_ctx).await;
        // debug_assert!(query_ctx.is_completed(),
        //     "Range query still not completed: {:?}", query_ctx); // Allow empty read

        // Handle negative records
        let mut searched_records: Vec<Record> = searched_records
            .into_iter()
            .map(|r| {
                if r.is_negative() {
                    self.cleaner.find_migrant(r.lba()).unwrap()
                } else {
                    r
                }
            })
            .collect();

        // Sort and group records' hbas in consecutive increasing order
        let record_batches = {
            searched_records.sort_by(|r1, r2| r1.hba().cmp(&r2.hba()));
            searched_records.group_by(|r1, r2| r2.hba().to_raw() - r1.hba().to_raw() == 1)
        };

        // Perform disk read in batches
        let mut blocks_buf =
            unsafe { Box::new_uninit_slice(searched_records.len() * BLOCK_SIZE).assume_init() };
        for record_batch in record_batches {
            let buf_len = record_batch.len() * BLOCK_SIZE;
            let read_len = self
                .disk
                .read(
                    record_batch.first().unwrap().hba().to_offset(),
                    &mut blocks_buf[..buf_len],
                )
                .await?;
            if read_len < buf_len {
                // Do not allow partial holed read in a batch read
                return_errno!(EIO, "failed to read expected length in batch read");
            }

            let mut offset = 0;
            for record in record_batch {
                let decrypted = DefaultCryptor::decrypt_block_aead(
                    &blocks_buf[offset..offset + BLOCK_SIZE],
                    &self.checkpoint.key_table().fetch_key(record.hba()),
                    record.cipher_meta(),
                )?;
                offset += BLOCK_SIZE;

                let idx = query_ctx.idx(record.lba());
                buf[idx * BLOCK_SIZE..(idx + 1) * BLOCK_SIZE].copy_from_slice(&decrypted);
            }
        }

        Ok(())
    }

    /// Read a single block into the given `buf`.
    async fn write_one_block(&self, bid: Bid, buf: &[u8]) -> Result<usize> {
        let target_lba = bid as Lba;

        // Write to data cache
        self.data_cache.insert(target_lba, buf).await?;

        Ok(buf.len())
    }

    /// Check if the arguments for a read or write is valid.
    fn check_rw_args(&self, offset: usize, buf: &[u8]) -> Result<()> {
        if offset + buf.len() > self.total_bytes() {
            return_errno!(EINVAL, "read/write length exceeds total bytes limit");
        } else if offset % BLOCK_SIZE != 0 || buf.len() % BLOCK_SIZE != 0 {
            return_errno!(
                EINVAL,
                "offset or buffer length not aligned with block size"
            );
        } else {
            Ok(())
        }
    }

    /// Disgard a bunch of lbas. Mark these blocks invalid so they can be cleaned later.
    #[allow(unused)]
    pub async fn discard(&self, lbas: &[Lba]) -> Result<usize> {
        // Insert negative records for each discard lba to lsm tree
        for &lba in lbas {
            self.lsm_tree.insert(lba, Record::new_negative(lba)).await?;
        }
        // TODO: Evaluate performance impact brought by discard
        Ok(lbas.len())
    }
}

#[async_trait]
impl BlockDeviceAsFile for JinDisk {
    fn total_bytes(&self) -> usize {
        self.data_blocks() * BLOCK_SIZE
    }

    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.read(offset, buf).await
    }

    async fn write(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        self.write(offset, buf).await
    }

    async fn sync(&self) -> Result<()> {
        self.sync().await
    }

    #[allow(unused)]
    async fn flush_blocks(&self, blocks: &[block_device::Bid]) -> Result<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use crate::index::LsmLevel;

    use super::*;
    use block_device::mem_disk::MemDisk;

    #[allow(unused)]
    fn create_new_jindisk() -> JinDisk {
        let total_blocks = 2 * GiB / BLOCK_SIZE;
        let mem_disk = MemDisk::new(total_blocks).unwrap();
        let root_key = DefaultCryptor::gen_random_key();
        JinDisk::create(Arc::new(mem_disk), root_key)
    }

    #[test]
    fn minor_compaction() -> Result<()> {
        async_rt::task::block_on(async move {
            let jindisk = create_new_jindisk();

            let rw_cnt = MAX_MEM_TABLE_CAPACITY;
            for i in 0..rw_cnt {
                let wbuf = [i as u8; BLOCK_SIZE];
                jindisk.write(i * BLOCK_SIZE, &wbuf).await?;
            }

            let bitc = jindisk.checkpoint().bitc().read();
            assert_eq!(bitc.l0_bit().is_some(), true);
            assert_eq!(
                bitc.find_bit_by_lba(Lba::new(0), 1 as LsmLevel).is_none(),
                true
            );
            Ok(())
        })
    }

    #[test]
    fn major_compaction() -> Result<()> {
        async_rt::task::block_on(async move {
            let jindisk = create_new_jindisk();

            let rw_cnt = MAX_MEM_TABLE_CAPACITY * 2;
            for i in 0..rw_cnt {
                let wbuf = [i as u8; BLOCK_SIZE];
                jindisk.write(i * BLOCK_SIZE, &wbuf).await?;
            }

            let bitc = jindisk.checkpoint().bitc().read();
            assert_eq!(
                bitc.find_bit_by_lba(Lba::new(0), 0 as LsmLevel).is_none(),
                true
            );
            assert_eq!(
                bitc.find_bit_by_lba(Lba::new(0), 1 as LsmLevel).is_some(),
                true
            );
            assert_eq!(
                bitc.find_bit_by_lba(Lba::from_byte_offset(1 * DATA_SIZE_PER_BIT), 0 as LsmLevel)
                    .is_some(),
                true
            );
            Ok(())
        })
    }

    #[test]
    fn segment_cleaning() -> Result<()> {
        async_rt::task::block_on(async move {
            let jindisk = create_new_jindisk();

            let rw_cnt: usize = DATA_SIZE_PER_BIT / BLOCK_SIZE;
            for _ in 0..5 {
                let wbuf = [0u8; BLOCK_SIZE];
                for i in 0..rw_cnt {
                    jindisk.write(i * BLOCK_SIZE, &wbuf).await?;
                }
            }

            // Succeed to log blocks over total blocks thanks to segment cleaning
            Ok(())
        })
    }
}
