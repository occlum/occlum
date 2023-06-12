//! Builder for BIT.
use super::super::record::{InternalRecord, LeafRecord, Record, RootRecord};
use super::super::LsmLevel;
use super::cache::{BitBuf, BitCache, BIT_CACHE_CAPACITY};
use super::{disk_bit::*, Bit, BitVersion};
use crate::prelude::*;

use std::convert::TryInto;

/// A builder for BIT.
pub struct BitBuilder {
    /// Start hba of the built BIT
    addr: Hba,
    /// Cryption key of the built BIT
    key: Key,
    /// A buffer to cache the on-disk BIT content
    buf: BitBuf,
    /// Node cache of BIT
    cache: Arc<BitCache>,
}

impl BitBuilder {
    pub fn new(addr: Hba) -> Self {
        Self {
            addr,
            key: DefaultCryptor::gen_random_key(),
            buf: BitBuf::new(),
            cache: Arc::new(BitCache::new(BIT_CACHE_CAPACITY)),
        }
    }

    /// Build a BIT with an array of records.
    ///
    /// On success, return a `Bit` which carries a `DiskBit` and a `BitCache`.
    pub async fn build(
        &mut self,
        records: &[Record],
        disk: &DiskView,
        level: LsmLevel,
        version: BitVersion,
    ) -> Result<Bit> {
        assert!(
            records.len() == MAX_RECORD_NUM_PER_BIT,
            "wrong records amount: {}",
            records.len()
        );

        // Build leaf blocks and construct leaf records
        let leaf_records = self.build_leafs(records);

        // Build internal blocks and construct internal records
        let internal_records = self.build_internals(&leaf_records);

        // Build root block and construct a BIT
        let disk_bit = self.build_bit(&internal_records, level, version);

        // Write back BIT content to disk
        disk.write(self.addr, self.buf.as_slice()).await?;

        Ok(Bit::new(disk_bit, self.cache.clone()))
    }

    /// Build a BIT with an array of internal records.
    fn build_bit(
        &mut self,
        internal_records: &[InternalRecord],
        level: u8,
        version: u32,
    ) -> DiskBit {
        debug_assert!(internal_records.len() == MAX_INTERNAL_RECORD_NUM_PER_ROOT);

        let (start, end) = (
            internal_records
                .first()
                .map(|record| record.lba_range().start())
                .unwrap(),
            internal_records
                .last()
                .map(|record| record.lba_range().end())
                .unwrap(),
        );
        let lba_range = LbaRange::new(start..end);

        let root_block = RootBlock::new(internal_records.to_vec().try_into().unwrap());

        // Encode and encrypt root node
        let mut encoded_root_block = [0u8; ROOT_BLOCK_SIZE];
        root_block.encode(&mut encoded_root_block).unwrap();
        let mut cipher_root_block = [0u8; ROOT_BLOCK_SIZE];
        let cipher_meta = DefaultCryptor::encrypt_arbitrary_aead(
            &encoded_root_block,
            &mut cipher_root_block,
            &self.key,
        );

        self.buf.as_slice_mut()[ROOT_REGION_OFFSET..ROOT_REGION_OFFSET + ROOT_BLOCK_SIZE]
            .copy_from_slice(&cipher_root_block);

        let root_record = RootRecord::new(
            lba_range,
            self.addr + Hba::from_byte_offset(ROOT_REGION_OFFSET).to_raw(),
            cipher_meta,
        );

        // Prepare root block of cache
        self.cache.set_root_block(Arc::new(root_block));

        DiskBit::new(self.addr, version, level, root_record, self.key)
    }

    /// Build internal records and blocks with an array of leaf records.
    fn build_internals(&mut self, leaf_records: &[LeafRecord]) -> Vec<InternalRecord> {
        debug_assert!(leaf_records.len() == MAX_LEAF_RECORD_NUM_PER_BIT);

        let mut internal_blocks = Vec::with_capacity(MAX_INTERNAL_RECORD_NUM_PER_ROOT);
        let mut internal_records = Vec::with_capacity(MAX_INTERNAL_RECORD_NUM_PER_ROOT);

        let mut offset = INTERNAL_REGION_OFFSET;
        for sub_records in leaf_records.chunks(MAX_INTERNAL_RECORD_NUM_PER_ROOT) {
            // TODO: Try avoid memory copy here
            let internal_block = InternalBlock::new(sub_records.to_vec().try_into().unwrap());

            let (start, end) = (
                sub_records
                    .first()
                    .map(|record| record.lba_range().start())
                    .unwrap(),
                sub_records
                    .last()
                    .map(|record| record.lba_range().end())
                    .unwrap(),
            );
            let lba_range = LbaRange::new(start..end);

            // Encode and encrypt internal node
            let mut encoded_internal_block = [0u8; INTERNAL_BLOCK_SIZE];
            internal_block.encode(&mut encoded_internal_block).unwrap();
            let mut cipher_internal_block = [0u8; INTERNAL_BLOCK_SIZE];
            let cipher_meta = DefaultCryptor::encrypt_arbitrary_aead(
                &encoded_internal_block,
                &mut cipher_internal_block,
                &self.key,
            );

            let internal_record = InternalRecord::new(
                lba_range,
                Hba::from_byte_offset_aligned(offset).unwrap() + self.addr.to_raw(),
                cipher_meta,
            );

            internal_blocks.push(internal_block);
            internal_records.push(internal_record);

            self.buf.as_slice_mut()[offset..offset + INTERNAL_BLOCK_SIZE]
                .copy_from_slice(&cipher_internal_block);

            offset += BLOCK_SIZE;
        }

        // Prepare internal blocks of cache
        self.cache.set_internal_blocks(Arc::new(internal_blocks));

        internal_records
    }

    /// Build leaf records and blocks with an array of records.
    fn build_leafs(&mut self, records: &[Record]) -> Vec<LeafRecord> {
        debug_assert!(records.len() == MAX_RECORD_NUM_PER_BIT);

        let mut leaf_records_blocks = Vec::with_capacity(MAX_LEAF_RECORD_NUM_PER_BIT);

        let mut offset = LEAF_REGION_OFFSET;
        for sub_records in records.chunks(MAX_RECORD_NUM_PER_LEAF) {
            // TODO: Try avoid memory copy here
            let leaf_block = LeafBlock::new(sub_records.to_vec().try_into().unwrap());

            let (start, end) = (
                sub_records.first().map(|record| record.lba()).unwrap(),
                sub_records.last().map(|record| record.lba()).unwrap() + 1 as _,
            );
            let lba_range = LbaRange::new(start..end);

            // Encode and encrypt leaf node
            let mut encoded_leaf_block = [0u8; LEAF_BLOCK_SIZE];
            leaf_block.encode(&mut encoded_leaf_block).unwrap();
            let cipher_leaf_block =
                DefaultCryptor::encrypt_block_aead(&encoded_leaf_block, &self.key);

            let leaf_record = LeafRecord::new(
                lba_range,
                Hba::from_byte_offset_aligned(offset).unwrap() + self.addr.to_raw(),
                CipherMeta::new(cipher_leaf_block.cipher_meta().mac().clone()),
            );

            self.buf.as_slice_mut()[offset..offset + LEAF_BLOCK_SIZE]
                .copy_from_slice(cipher_leaf_block.as_slice());

            offset += LEAF_BLOCK_SIZE;

            leaf_records_blocks.push((leaf_record, Arc::new(leaf_block)));
        }

        let leaf_records = leaf_records_blocks
            .iter()
            .map(|(leaf_record, _)| leaf_record.clone())
            .collect();

        // Prepare leaf blocks of cache
        self.cache.put_leaf_blocks(leaf_records_blocks);

        leaf_records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::bit::BitId;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn bit_build() -> Result<()> {
        async_rt::task::block_on(async move {
            let records = vec![Record::new_uninit(); MAX_MEM_TABLE_CAPACITY];
            let mem_disk = MemDisk::new(1024usize).unwrap();
            let disk = DiskView::new_unchecked(Arc::new(mem_disk));

            let level = 1 as LsmLevel;
            let version = 10 as BitVersion;
            let bit = BitBuilder::new(Hba::new(1))
                .build(&records, &disk, level, version)
                .await?;

            assert_eq!(bit.id(), BitId::new(1));
            assert_eq!(bit.level(), level);
            assert_eq!(bit.version(), version);
            Ok(())
        })
    }
}
