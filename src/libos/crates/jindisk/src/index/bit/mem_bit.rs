//! Metadata for BITs of Lsm Tree.
use super::super::lsm_tree::LsmLevel;
use super::super::record::{InternalRecord, LeafRecord, Record, RootRecord};
use super::cache::{BitCache, BIT_CACHE_CAPACITY};
use super::disk_bit::*;
use super::{BitId, BitVersion};
use crate::prelude::*;
use crate::util::RangeQueryCtx;

use std::fmt::{self, Debug};

/// Block Index Table (in-memory).
///
/// It manages the underlying on-disk BIT and BIT node cache.
#[derive(Clone)]
pub struct Bit {
    bit: DiskBit,
    cache: Arc<BitCache>,
}

impl Bit {
    pub fn new(bit: DiskBit, cache: Arc<BitCache>) -> Self {
        Self { bit, cache }
    }

    pub fn lba_range(&self) -> &LbaRange {
        self.bit.lba_range()
    }

    pub fn id(&self) -> BitId {
        self.bit.id()
    }

    pub fn version(&self) -> BitVersion {
        self.bit.version()
    }

    pub fn key(&self) -> &Key {
        &self.bit.key()
    }

    pub fn level(&self) -> LsmLevel {
        self.bit.level()
    }

    #[allow(unused)]
    pub fn addr(&self) -> Hba {
        self.id() as _
    }

    #[allow(unused)]
    pub fn bit(&self) -> &DiskBit {
        &self.bit
    }

    #[allow(unused)]
    pub fn cache(&self) -> &Arc<BitCache> {
        &self.cache
    }

    // Test-purpose
    #[allow(unused)]
    pub(crate) fn new_unchecked(id: BitId, lba_range: LbaRange) -> Self {
        Self {
            bit: DiskBit::new_unchecked(id, lba_range),
            cache: Arc::new(BitCache::new(BIT_CACHE_CAPACITY)),
        }
    }
}

impl Bit {
    /// Search the target record in BIT given a lba and a flag.
    pub async fn search(&self, target_lba: Lba, disk: &DiskView) -> Option<Record> {
        debug_assert!(self.lba_range().is_within_range(target_lba));
        self.search_with(target_lba, disk, SearchFlag::Cached).await
    }

    async fn search_with(
        &self,
        target_lba: Lba,
        disk: &DiskView,
        flag: SearchFlag,
    ) -> Option<Record> {
        debug_assert!(self.lba_range().is_within_range(target_lba));
        match flag {
            SearchFlag::Direct => self.search_direct(target_lba, disk).await,
            SearchFlag::Cached => self.search_cached(target_lba, disk).await,
        }
    }

    /// Fast path (cache)
    async fn search_cached(&self, target_lba: Lba, disk: &DiskView) -> Option<Record> {
        // Search leaf record from cache
        let leaf_record = self.cache.search_leaf_record(target_lba)?;

        // Get leaf block from cache or disk
        let mut contained_incache = false;
        let leaf_block = if let Some(block) = self.cache.get_leaf_block(&leaf_record) {
            contained_incache = true;
            block
        } else {
            Arc::new(
                self.get_leaf_block_from_disk(&leaf_record, disk)
                    .await
                    .unwrap(),
            )
        };

        // Search leaf block
        let target_records = leaf_block.records();
        if let Ok(pos) = target_records.binary_search_by(|record| record.lba().cmp(&target_lba)) {
            let searched_record = if target_records[pos].is_negative()
                && target_records[pos.saturating_sub(1)].lba() == target_lba
            {
                // Check the one before the negative one
                target_records[pos.saturating_sub(1)].clone()
            } else {
                target_records[pos].clone()
            };
            if !contained_incache {
                self.cache.put_leaf_block(leaf_record.clone(), leaf_block);
            }
            return Some(searched_record);
        }
        None
    }

    // Slow path (disk)
    async fn search_direct(&self, target_lba: Lba, disk: &DiskView) -> Option<Record> {
        let root_block = self
            .get_root_block_from_disk(&self.bit.root_record(), disk)
            .await
            .ok()?;

        // Search level 1
        for internal_record in root_block.internal_records() {
            if !internal_record.lba_range().is_within_range(target_lba) {
                continue;
            }

            let internal_block = self
                .get_internal_block_from_disk(&internal_record, disk)
                .await
                .ok()?;

            // Search level 2
            for leaf_record in internal_block.leaf_records() {
                if !leaf_record.lba_range().is_within_range(target_lba) {
                    continue;
                }

                let leaf_block = self
                    .get_leaf_block_from_disk(&leaf_record, disk)
                    .await
                    .ok()?;

                // Search leaf block
                let target_records = leaf_block.records();
                if let Ok(pos) =
                    target_records.binary_search_by(|record| record.lba().cmp(&target_lba))
                {
                    let searched_record = if target_records[pos].is_negative()
                        && target_records[pos.saturating_sub(1)].lba() == target_lba
                    {
                        // Check the one before the negative one
                        target_records[pos.saturating_sub(1)].clone()
                    } else {
                        target_records[pos].clone()
                    };
                    return Some(searched_record);
                }
            }
        }
        None
    }

    /// Fast path (cache)
    pub async fn search_range(
        &self,
        query_ctx: &mut RangeQueryCtx,
        disk: &DiskView,
        searched_records: &mut Vec<Record>,
    ) {
        let mut iter = query_ctx.collect_uncompleted().into_iter();
        // Search record by each lba in the query
        loop {
            let ctx = iter.next();
            if ctx.is_none() {
                break;
            }
            let (_, target_lba) = ctx.unwrap();
            if !self.lba_range().is_within_range(target_lba) {
                continue;
            }

            if let Some(record) = self.search(target_lba, disk).await {
                searched_records.push(record);
                query_ctx.complete(target_lba);
            }
        }
    }

    /// Collect all records from BIT.
    pub async fn collect_all_records(&self, disk: &DiskView) -> Result<Vec<Record>> {
        // TODO: Pass reference of result vector
        self.collect_all_records_with(disk, SearchFlag::Cached)
            .await
    }

    async fn collect_all_records_with(
        &self,
        disk: &DiskView,
        flag: SearchFlag,
    ) -> Result<Vec<Record>> {
        match flag {
            SearchFlag::Direct => self.collect_all_records_direct(disk).await,
            SearchFlag::Cached => self.collect_all_records_cached(disk).await,
        }
    }

    // Fast path (cache)
    async fn collect_all_records_cached(&self, disk: &DiskView) -> Result<Vec<Record>> {
        let mut all_records = Vec::with_capacity(MAX_RECORD_NUM_PER_BIT);

        let internal_blocks = self.cache.internal_blocks();

        // Traverse level 1
        for internal_block in internal_blocks.iter() {
            // Traverse level 2
            for leaf_record in internal_block.leaf_records() {
                let leaf_block = if let Some(block) = self.cache.peek_leaf_block(leaf_record) {
                    block
                } else {
                    Arc::new(
                        self.get_leaf_block_from_disk(leaf_record, disk)
                            .await
                            .unwrap(),
                    )
                };

                let mut records = leaf_block.records().to_vec();
                {
                    // Deal with duplication
                    let len = records.len();
                    if records[len.saturating_sub(1)] == records[len.saturating_sub(2)] {
                        records.dedup();
                        all_records.extend(records);
                        return Ok(all_records);
                    }
                }
                all_records.extend(records);
            }
        }

        Ok(all_records)
    }

    // Slow path (disk)
    async fn collect_all_records_direct(&self, disk: &DiskView) -> Result<Vec<Record>> {
        let mut all_records = Vec::with_capacity(MAX_RECORD_NUM_PER_BIT);

        let root_block = self
            .get_root_block_from_disk(self.bit.root_record(), disk)
            .await?;

        // Traverse level 1
        'outer: for internal_record in root_block.internal_records() {
            let internal_block = self
                .get_internal_block_from_disk(&internal_record, disk)
                .await?;

            // Traverse level 2
            for leaf_record in internal_block.leaf_records() {
                let leaf_block = self.get_leaf_block_from_disk(&leaf_record, disk).await?;

                // Deal with duplication
                let mut records = leaf_block.records().to_vec();
                records.dedup();
                let tailored_len = records.len();
                all_records.extend(records);

                if tailored_len < MAX_RECORD_NUM_PER_LEAF {
                    break 'outer;
                }
            }
        }

        Ok(all_records)
    }

    #[allow(unaligned_references)]
    pub async fn get_root_block_from_disk(
        &self,
        root_record: &RootRecord,
        disk: &DiskView,
    ) -> Result<RootBlock> {
        // Read from disk
        let root_block_addr = root_record.hba();
        let mut rbuf = [0u8; BLOCK_SIZE];
        disk.read(root_block_addr, &mut rbuf).await?;

        // Decrypt and decode
        let mut decrypted = [0u8; ROOT_BLOCK_SIZE];
        DefaultCryptor::decrypt_arbitrary_aead(
            &rbuf[0..ROOT_BLOCK_SIZE],
            &mut decrypted,
            self.key(),
            root_record.cipher_meta(),
        )?;

        RootBlock::decode(&decrypted)
    }

    #[allow(unaligned_references)]
    pub async fn get_internal_block_from_disk(
        &self,
        internal_record: &InternalRecord,
        disk: &DiskView,
    ) -> Result<InternalBlock> {
        // Read from disk
        let internal_block_addr = internal_record.hba();
        let mut rbuf = [0u8; BLOCK_SIZE];
        disk.read(internal_block_addr, &mut rbuf).await?;

        // Decrypt and decode
        let mut decrypted = [0u8; INTERNAL_BLOCK_SIZE];
        DefaultCryptor::decrypt_arbitrary_aead(
            &rbuf[0..INTERNAL_BLOCK_SIZE],
            &mut decrypted,
            self.key(),
            internal_record.cipher_meta(),
        )?;

        InternalBlock::decode(&decrypted)
    }

    #[allow(unaligned_references)]
    pub async fn get_leaf_block_from_disk(
        &self,
        leaf_record: &LeafRecord,
        disk: &DiskView,
    ) -> Result<LeafBlock> {
        // Read from disk
        let leaf_block_addr = leaf_record.hba();
        let mut rbuf = [0u8; LEAF_BLOCK_SIZE];
        disk.read(leaf_block_addr, &mut rbuf).await?;

        // Decrypt and decode
        let decrypted =
            DefaultCryptor::decrypt_block_aead(&rbuf, self.key(), leaf_record.cipher_meta())?;

        LeafBlock::decode(&decrypted)
    }

    /// Initialize cache (Read root and internal nodes from disk).
    pub async fn init_cache(&self, disk: &DiskView) -> Result<()> {
        let root_block = self
            .get_root_block_from_disk(self.bit.root_record(), disk)
            .await?;
        let mut internal_blocks = Vec::with_capacity(MAX_INTERNAL_RECORD_NUM_PER_ROOT);
        for internal_record in root_block.internal_records() {
            let internal_block = self
                .get_internal_block_from_disk(internal_record, disk)
                .await?;
            internal_blocks.push(internal_block);
        }

        self.cache.set_root_block(Arc::new(root_block));
        self.cache.set_internal_blocks(Arc::new(internal_blocks));
        Ok(())
    }
}

impl Debug for Bit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BIT (Block Index Table)")
            .field("id", &self.id())
            .field("version", &self.version())
            .field("lba_range", self.lba_range())
            .finish()
    }
}

impl Serialize for Bit {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        self.bit.encode(encoder)
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            bit: DiskBit::decode(buf)?,
            cache: Arc::new(BitCache::new(BIT_CACHE_CAPACITY)),
        })
    }
}

/// Search flag. Indicate search strategy of BIT.
#[derive(Clone)]
enum SearchFlag {
    /// Search on disk directly.
    #[allow(unused)]
    Direct,
    /// Search through BIT node cache.
    Cached,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::bit::BitBuilder;
    use sgx_disk::{HostDisk, SyncIoDisk};

    use std::time::Instant;

    #[test]
    fn bit_search() -> Result<()> {
        async_rt::task::block_on(async move {
            let total_blocks = 1024usize;
            let path = "bit_test0.image";
            let disk = Arc::new(SyncIoDisk::create(&path, total_blocks).unwrap());
            let disk = DiskView::new_unchecked(disk);

            let records = vec![Record::new_uninit(); MAX_MEM_TABLE_CAPACITY];
            let bit = BitBuilder::new(Hba::new(0))
                .build(&records, &disk, 0 as LsmLevel, 0 as BitVersion)
                .await
                .unwrap();

            let target_lba = Lba::new(0);
            let cnt = 1000;

            let start = Instant::now();
            for _ in 0..cnt {
                let target_record = bit
                    .search_with(target_lba, &disk, SearchFlag::Direct)
                    .await
                    .unwrap();
                assert_eq!(target_record.lba(), target_lba);
            }
            let duration = start.elapsed();
            println!("Time elapsed in bit_search() (direct) is: {:?}", duration);

            let start = Instant::now();
            for _ in 0..cnt {
                let target_record = bit
                    .search_with(target_lba, &disk, SearchFlag::Cached)
                    .await
                    .unwrap();
                assert_eq!(target_record.lba(), target_lba);
            }
            let duration = start.elapsed();
            println!("Time elapsed in bit_search() (cached) is: {:?}", duration);

            let _ = std::fs::remove_file(&path);
            Ok(())
        })
    }

    #[test]
    fn bit_search_range() -> Result<()> {
        async_rt::task::block_on(async move {
            let total_blocks = 1024usize;
            let path = "bit_test1.image";
            let disk = Arc::new(SyncIoDisk::create(&path, total_blocks).unwrap());
            let disk = DiskView::new_unchecked(disk);
            let cap = MAX_MEM_TABLE_CAPACITY;

            let mut records = Vec::with_capacity(cap);
            for i in 0..cap {
                records.push(Record::new(
                    Lba::new(i as _),
                    Hba::new(0),
                    CipherMeta::new_uninit(),
                ));
            }

            let bit = BitBuilder::new(Hba::new(0))
                .build(&records, &disk, 0 as LsmLevel, 0 as BitVersion)
                .await
                .unwrap();

            let range_len = 128usize;
            for lba in [Lba::new(0), Lba::new(64), Lba::new(137), Lba::new(1225)] {
                let mut range = (lba, vec![0u8; range_len * BLOCK_SIZE]);
                let mut query_ctx = RangeQueryCtx::build_from(range.0.to_offset(), &mut range.1);
                let mut searched_records = Vec::new();

                bit.search_range(&mut query_ctx, &disk, &mut searched_records)
                    .await;
                assert_eq!(query_ctx.is_completed(), true);
                assert_eq!(searched_records.len(), range_len);
                assert_eq!(searched_records[0].lba(), lba);
            }

            let _ = std::fs::remove_file(&path);
            Ok(())
        })
    }

    #[test]
    fn bit_collect_all_records() -> Result<()> {
        async_rt::task::block_on(async move {
            let total_blocks = 1024usize;
            let records = vec![Record::new_uninit(); MAX_MEM_TABLE_CAPACITY];
            let path = "bit_test2.image";
            let disk = Arc::new(SyncIoDisk::create(&path, total_blocks).unwrap());
            let disk = DiskView::new_unchecked(disk);

            let bit = BitBuilder::new(Hba::new(0))
                .build(&records, &disk, 0 as LsmLevel, 0 as BitVersion)
                .await
                .unwrap();

            let cnt = 1000;

            let start = Instant::now();
            for _ in 0..cnt {
                let _ = bit
                    .collect_all_records_with(&disk, SearchFlag::Direct)
                    .await
                    .unwrap();
            }
            let duration = start.elapsed();
            println!(
                "Time elapsed in collect_all_records() (direct) is: {:?}",
                duration
            );

            let start = Instant::now();
            for _ in 0..cnt {
                let _ = bit
                    .collect_all_records_with(&disk, SearchFlag::Cached)
                    .await
                    .unwrap();
            }
            let duration = start.elapsed();
            println!(
                "Time elapsed in collect_all_records() (cached) is: {:?}",
                duration
            );

            let _ = std::fs::remove_file(&path);
            Ok(())
        })
    }
}
