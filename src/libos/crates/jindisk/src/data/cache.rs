//! Data cache subsystem for caching read/write plain data.
use super::CacheState;
use crate::prelude::*;
use crate::util::RangeQueryCtx;
use crate::{Checkpoint, LsmTree, Record};

use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A cache for data. It consists of a buffer pool to manage
/// multi segment buffers.
pub struct DataCache {
    buffer_pool: Vec<Arc<SegmentBuffer>>,
    current_idx: AtomicUsize,
    capacity: usize,
    arw_lock: AsyncRwLock<()>,
}

/// Segment buffer. It caches and manages plain data blocks of one segment.
pub struct SegmentBuffer {
    plain_data_blocks: RwLock<HashMap<Lba, DataBlock>>,
    state: Mutex<CacheState>,
    segment_addr: Mutex<Option<Hba>>,
    capacity: usize,
    disk: DiskView,
    checkpoint: Arc<Checkpoint>,
    pollee: Pollee,
}

impl DataCache {
    /// Initialize a `DataCache` given a capacity of pool.
    pub fn new(pool_capacity: usize, disk: DiskView, checkpoint: Arc<Checkpoint>) -> Self {
        Self {
            buffer_pool: {
                let mut pool = Vec::with_capacity(pool_capacity);
                for _ in 0..pool_capacity {
                    pool.push(Arc::new(SegmentBuffer::new(
                        SEGMENT_BUFFER_CAPACITY,
                        disk.clone(),
                        checkpoint.clone(),
                    )))
                }
                pool
            },
            current_idx: AtomicUsize::new(0),
            capacity: pool_capacity,
            arw_lock: AsyncRwLock::new(()),
        }
    }

    /// Insert a block buffer to cache.
    pub async fn insert(&self, lba: Lba, buf: &[u8], lsm_tree: Arc<LsmTree>) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;

        self.insert_block(lba, buf, lsm_tree).await?;

        drop(aw_lock);
        Ok(())
    }

    /// Search the data block from a target lba.
    pub async fn search(&self, target_lba: Lba, buf: &mut [u8]) -> Result<usize> {
        let ar_lock = self.arw_lock.read().await;

        let cap = self.capacity;
        let cur = self.current_idx.load(Ordering::SeqCst);

        // Search ordered by access time
        for i in (0..cur + 1)
            .into_iter()
            .rev()
            .chain((cur + 1..cap).into_iter().rev())
        {
            let seg_buf = &self.buffer_pool[i];
            if let Ok(buf_len) = seg_buf.search(target_lba, buf) {
                return Ok(buf_len);
            }
        }

        drop(ar_lock);
        Err(errno!(ENOENT, "search not found"))
    }

    /// Range search multi data blocks from consecutive lbas.
    /// Blocks are read into `buf`.
    pub async fn search_range(&self, query_ctx: &mut RangeQueryCtx, buf: &mut [u8]) {
        let ar_lock = self.arw_lock.read().await;

        let cap = self.capacity;
        let cur = self.current_idx.load(Ordering::SeqCst);

        // Search ordered by access time
        for i in (0..cur + 1)
            .into_iter()
            .rev()
            .chain((cur + 1..cap).into_iter().rev())
        {
            let seg_buf = &self.buffer_pool[i];
            seg_buf.search_range(query_ctx, buf).await;
            if query_ctx.is_completed() {
                return;
            }
        }

        drop(ar_lock);
    }

    /// Insert one data block to cache.
    async fn insert_block(&self, lba: Lba, buf: &[u8], lsm_tree: Arc<LsmTree>) -> Result<()> {
        let cur_idx = self.current_idx.load(Ordering::SeqCst);
        let current_buf = &self.buffer_pool[cur_idx];

        // Allocate and bind a data segment
        current_buf.alloc_segment();

        let is_full = current_buf.insert(lba, buf).await?;

        // If the buffer is full, change current index of pool
        // and start a writeback task
        if is_full {
            self.current_idx
                .store((cur_idx + 1) % self.capacity, Ordering::SeqCst);

            // Background writeback
            let cur_buf = current_buf.clone();
            // Clear events
            cur_buf.clear_events();
            // TODO: Fix timing sequence across each segment buffer
            #[cfg(feature = "sgx")]
            async_rt::task::spawn(async move {
                cur_buf.encrypt_and_persist(lsm_tree).await.unwrap();
            });
            // Foreground writeback (Test-purpose)
            #[cfg(not(feature = "sgx"))]
            cur_buf.encrypt_and_persist(lsm_tree).await?;
        }
        Ok(())
    }

    /// Search or insert a data block to cache.
    /// Used for segment cleaning, must be atomic.
    pub async fn search_or_insert(
        &self,
        lba: Lba,
        buf: &[u8],
        lsm_tree: Arc<LsmTree>,
    ) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;

        let mut has_newer_data = false;
        // Search each segment buffer
        for seg_buf in &self.buffer_pool {
            if seg_buf.check_newer(lba, lsm_tree.clone()).await {
                has_newer_data = true;
                break;
            }
        }
        if has_newer_data {
            // Newer data are written, no need to migrate old one
            return Ok(());
        }

        // No newer data found, migrate this block
        self.insert_block(lba, buf, lsm_tree).await?;

        drop(aw_lock);
        Ok(())
    }

    pub async fn persist(&self) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;

        // TODO: Flush partial filled `SegmentBuffer` first
        for seg_buf in &self.buffer_pool {
            seg_buf.wait_writeback().await?;
        }

        drop(aw_lock);
        Ok(())
    }
}

impl SegmentBuffer {
    /// Initialize a `SegmentBuffer` given a capacity.
    pub fn new(capacity: usize, disk: DiskView, checkpoint: Arc<Checkpoint>) -> Self {
        Self {
            plain_data_blocks: RwLock::new(HashMap::new()),
            state: Mutex::new(CacheState::Vacant),
            segment_addr: Mutex::new(None),
            capacity,
            checkpoint,
            pollee: Pollee::new(Events::empty()),
            disk,
        }
    }

    /// Insert a data block (build by offset `lba` and block `buf`).
    pub async fn insert(&self, lba: Lba, buf: &[u8]) -> Result<bool> {
        let mut state = self.state.lock();
        let mut is_full: bool = false;

        loop {
            match *state {
                // `Vacant` indicates available to insert
                CacheState::Vacant => {
                    break;
                }
                // `Full | Flushing | Clearing` indicates current buffer is busy,
                // need to wait for a event (wait until it becomes `Vacant` again)
                CacheState::Full | CacheState::Flushing | CacheState::Clearing => {
                    drop(state);
                    self.wait_events(Events::OUT).await?;
                    state = self.state.lock();
                }
            }
        }

        debug_assert!(*state == CacheState::Vacant);
        let mut data_blocks = self.plain_data_blocks.write();
        // Insert and check if buffer becomes full
        if let Some(data_block) = data_blocks.get_mut(&lba) {
            data_block.as_slice_mut().copy_from_slice(buf);
        } else {
            data_blocks.insert(lba, DataBlock::from_buf(buf));
        }
        if data_blocks.len() >= self.capacity {
            *state = CacheState::Full;
            is_full = true;
        }
        drop(data_blocks);
        drop(state);

        Ok(is_full)
    }

    async fn wait_writeback(&self) -> Result<()> {
        let mut state = self.state.lock();

        loop {
            match *state {
                // `Vacant` indicates available to insert
                CacheState::Vacant => {
                    break;
                }
                // `Full | Flushing | Clearing` indicates current buffer is busy,
                // need to wait for a event (wait until it becomes `Vacant` again)
                CacheState::Full | CacheState::Flushing | CacheState::Clearing => {
                    drop(state);
                    self.wait_events(Events::OUT).await?;
                    state = self.state.lock();
                }
            }
        }

        drop(state);
        Ok(())
    }

    /// Search the data block from a target lba.
    pub fn search(&self, target_lba: Lba, buf: &mut [u8]) -> Result<usize> {
        let state = self.state.lock();

        match *state {
            // `Vacant | Full | Flushing` indicates available to read
            CacheState::Vacant | CacheState::Full | CacheState::Flushing => {
                if let Some(data_block) = self.plain_data_blocks.read().get(&target_lba) {
                    buf.copy_from_slice(data_block.as_slice());
                    drop(state);
                    return Ok(buf.len());
                }
            }
            // `Clearing` indicates target data block is being flushed and cleared.
            // We just return `None` and the data will be found next step (from index)
            CacheState::Clearing => {}
        }

        drop(state);
        Err(errno!(ENOENT, "search not found"))
    }

    /// Range search multi data blocks from consecutive lbas.
    /// Blocks are read into `buf`.
    pub async fn search_range(&self, query_ctx: &mut RangeQueryCtx, buf: &mut [u8]) {
        let state = self.state.lock();

        match *state {
            // `Vacant | Full | Flushing` indicates available to read
            CacheState::Vacant | CacheState::Full | CacheState::Flushing => {
                let data_blocks = self.plain_data_blocks.read();
                for (idx, lba) in query_ctx.collect_uncompleted() {
                    if let Some(data_block) = data_blocks.get(&lba) {
                        buf[idx..idx + BLOCK_SIZE].copy_from_slice(data_block.as_slice());
                        query_ctx.complete(lba);
                    }
                }
            }
            // `Clearing` indicates target data block is being flushed and cleared.
            // We just return `None` and the data will be found next step (from index)
            CacheState::Clearing => {}
        }

        drop(state);
    }

    /// Allocate and bind a segment to current segment buffer.
    fn alloc_segment(&self) {
        let mut seg_addr = self.segment_addr.lock();
        // Check if current segment is allocated
        if seg_addr.is_some() {
            return;
        }

        // Allocate and bind a segment
        // Assume to success every time due to segment cleaning policy
        let picked_seg = self.checkpoint.data_svt().write().pick_avail_seg().unwrap();

        // Insert to `DST`
        self.checkpoint.dst().write().validate_or_insert(picked_seg);

        let _ = seg_addr.insert(picked_seg);
    }

    /// Check if segment buffer already contains written block of target lba.
    async fn check_newer(&self, target_lba: Lba, lsm_tree: Arc<LsmTree>) -> bool {
        let state = self.state.lock();

        match *state {
            // `Vacant | Full | Flushing` indicates available to read
            CacheState::Vacant | CacheState::Full | CacheState::Flushing => {
                if self.plain_data_blocks.read().contains_key(&target_lba) {
                    return true;
                }
                drop(state);
            }
            // `Clearing` indicates the buffer is just being flushed.
            // We should check index immediately
            CacheState::Clearing => {
                if let Some(record) = lsm_tree.search(target_lba).await {
                    if !record.is_negative() {
                        return true;
                    }
                }
            }
        }

        false
    }

    async fn wait_events(&self, events: Events) -> Result<()> {
        let poller = Poller::new();
        if self.pollee.poll(events, Some(&poller)).is_empty() {
            poller.wait().await?;
        }
        Ok(())
    }

    fn clear_events(&self) {
        self.pollee.reset_events();
    }

    #[allow(unused)]
    fn set_state(&self, new_state: CacheState) {
        let mut state = self.state.lock();
        CacheState::examine_state_transition(*state, new_state);
        *state = new_state;
    }
}

impl SegmentBuffer {
    /// Write-back function. It triggers when current segment buffer becomes
    /// full. First, encrypt each blocks, second, build records and insert into index,
    /// then, persist cipher blocks to underlying disk.
    async fn encrypt_and_persist(&self, lsm_tree: Arc<LsmTree>) -> Result<()> {
        assert!(*self.state.lock() == CacheState::Full);
        *self.state.lock() = CacheState::Flushing;

        let data_blocks = self.plain_data_blocks.read();
        debug_assert!(data_blocks.len() == self.capacity);

        let seg_addr = self.segment_addr.lock().unwrap();
        let mut block_addr = seg_addr;

        // Sort each block by lba
        let mut sorted_pb = data_blocks.iter().collect::<Vec<_>>();
        sorted_pb.sort_by_key(|kv| kv.0);
        // Encrypt each block
        let mut sorted_cb = sorted_pb
            .iter()
            .map(|(&lba, data_block)| {
                (
                    lba,
                    DefaultCryptor::encrypt_block(
                        data_block.as_slice(),
                        &self.checkpoint.key_table().get_or_insert(block_addr),
                    ),
                )
            })
            .collect::<Vec<_>>();
        drop(data_blocks);

        let mut wbufs = Vec::with_capacity(self.capacity);
        let mut new_records = Vec::with_capacity(self.capacity);
        for (lba, cipher_block) in sorted_cb.iter_mut() {
            // Collect cipher block bufs
            // Safety.
            wbufs.push(unsafe {
                BlockBuf::from_raw_parts(
                    NonNull::new_unchecked(cipher_block.as_slice_mut().as_mut_ptr()),
                    BLOCK_SIZE,
                )
            });

            // Currently, no need to search index and collect older data blocks
            // thanks to delayed block reclamation policy.

            // Build new record
            new_records.push(Record::new(
                *lba,
                block_addr,
                cipher_block.cipher_meta().clone(),
            ));

            block_addr = block_addr + 1 as _;
        }

        // Write back cipher blocks to disk
        self.write_consecutive_blocks(seg_addr, wbufs).await?;

        for new_record in new_records {
            // Update RIT
            self.checkpoint
                .rit()
                .write()
                .await
                .insert(new_record.hba(), new_record.lba())
                .await?;
            // Update index only when data are successfully persisted
            lsm_tree.insert(new_record.lba(), new_record).await?;
        }

        // Update state and clear buffer
        *self.state.lock() = CacheState::Clearing;
        self.plain_data_blocks.write().clear();

        // Unbind segment
        self.segment_addr.lock().take();

        // Update state and notify a event
        *self.state.lock() = CacheState::Vacant;
        self.pollee.add_events(Events::OUT);
        Ok(())
    }

    async fn write_consecutive_blocks(&self, addr: Hba, write_bufs: Vec<BlockBuf>) -> Result<()> {
        debug_assert!(write_bufs.len() == SEGMENT_BUFFER_CAPACITY);

        let req = BioReqBuilder::new(BioType::Write)
            .addr(addr)
            .bufs(write_bufs)
            .build();
        let submission = self.disk.submit(Arc::new(req))?;
        let req = submission.complete().await;
        let res = req.response().unwrap();

        if let Err(e) = res {
            return Err(errno!(e.errno(), "write on a block device failed"));
        }
        Ok(())
    }
}

/// A wrapper struct to wrap a block buffer.
#[derive(Clone)]
pub struct DataBlock(Box<[u8]>);

impl DataBlock {
    pub fn from_buf(buf: &[u8]) -> Self {
        let mut boxed_slice = unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
        boxed_slice.copy_from_slice(buf);
        Self(boxed_slice)
    }

    pub fn new_uninit() -> Self {
        let boxed_slice = unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
        Self(boxed_slice)
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &*self.0
    }

    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut *self.0
    }
}
