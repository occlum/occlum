//! Index segment buffer for caching BIT.
use super::super::record::LeafRecord;
use super::disk_bit::{
    InternalBlock, LeafBlock, RootBlock, BIT_SIZE_ON_DISK, MAX_LEAF_RECORD_NUM_PER_BIT,
};
use crate::prelude::*;
use lru::LruCache;

// BIT cache capacity (leaf nodes)
pub const BIT_CACHE_CAPACITY: usize = MAX_LEAF_RECORD_NUM_PER_BIT;

/// A cache for BIT. It caches tree structure in memory.
/// The root and internal nodes are all cached, leaf nodes is LRU cached.
pub struct BitCache {
    root_block: RwLock<Option<Arc<RootBlock>>>,
    internal_blocks: RwLock<Arc<Vec<InternalBlock>>>,
    leaf_capacity: usize,
    lru_leaf_blocks: RwLock<LruCache<LeafRecord, Arc<LeafBlock>>>,
}

impl BitCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            root_block: RwLock::new(None),
            internal_blocks: RwLock::new(Arc::new(Vec::new())),
            lru_leaf_blocks: RwLock::new(LruCache::new(capacity)),
            leaf_capacity: capacity,
        }
    }

    #[allow(unused)]
    pub fn root_block(&self) -> Option<Arc<RootBlock>> {
        self.root_block.read().clone()
    }

    pub fn set_root_block(&self, root_block: Arc<RootBlock>) {
        let _ = self.root_block.write().insert(root_block);
    }

    pub fn internal_blocks(&self) -> Arc<Vec<InternalBlock>> {
        self.internal_blocks.read().clone()
    }

    pub fn set_internal_blocks(&self, internal_blocks: Arc<Vec<InternalBlock>>) {
        *self.internal_blocks.write() = internal_blocks;
    }

    /// Search (tree traversal) the target leaf record with a lba.
    pub fn search_leaf_record(&self, target_lba: Lba) -> Option<LeafRecord> {
        let root_guard = self.root_block.read();
        let root_block = root_guard.as_ref().unwrap();
        let internal_blocks = self.internal_blocks.read();

        // Search level 1
        if let Some(internal_pos) = root_block
            .internal_records()
            .iter()
            .position(|record| record.lba_range().is_within_range(target_lba))
        {
            let internal_block = &internal_blocks[internal_pos];

            // Search level 2
            if let Some(leaf_pos) = internal_block
                .leaf_records()
                .iter()
                .position(|record| record.lba_range().is_within_range(target_lba))
            {
                return Some(internal_block.leaf_records()[leaf_pos].clone());
            }
        }

        None
    }

    /// Get leaf block from lru leaf nodes (alter lru state).
    pub fn get_leaf_block(&self, leaf_record: &LeafRecord) -> Option<Arc<LeafBlock>> {
        let mut lru_leaf_blocks = self.lru_leaf_blocks.write();
        lru_leaf_blocks
            .get(leaf_record)
            .map(|leaf_block| leaf_block.clone())
    }

    /// Peek leaf block from lru leaf nodes (keep lru state unchanged).
    pub fn peek_leaf_block(&self, leaf_record: &LeafRecord) -> Option<Arc<LeafBlock>> {
        let lru_leaf_blocks = self.lru_leaf_blocks.read();
        lru_leaf_blocks
            .peek(leaf_record)
            .map(|leaf_block| leaf_block.clone())
    }

    pub fn put_leaf_block(&self, leaf_record: LeafRecord, leaf_block: Arc<LeafBlock>) {
        let mut lru_leaf_blocks = self.lru_leaf_blocks.write();
        lru_leaf_blocks.put(leaf_record, leaf_block);
    }

    pub fn put_leaf_blocks(&self, leaf_records_blocks: Vec<(LeafRecord, Arc<LeafBlock>)>) {
        let mut lru_leaf_blocks = self.lru_leaf_blocks.write();
        for (leaf_record, leaf_block) in leaf_records_blocks {
            lru_leaf_blocks.put(leaf_record, leaf_block);
        }
    }

    #[allow(unused)]
    pub fn size(&self) -> usize {
        self.lru_leaf_blocks.read().len()
    }

    #[allow(unused)]
    pub fn capacity(&self) -> usize {
        self.leaf_capacity
    }

    #[allow(unused)]
    pub fn contains(&self, leaf_record: &LeafRecord) -> bool {
        self.lru_leaf_blocks.read().contains(leaf_record)
    }
}

// A buffer for caching BIT.
pub struct BitBuf(Box<[u8]>);

impl BitBuf {
    pub fn new() -> Self {
        let boxed_slice = unsafe { Box::new_uninit_slice(BIT_SIZE_ON_DISK).assume_init() };
        Self(boxed_slice)
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
