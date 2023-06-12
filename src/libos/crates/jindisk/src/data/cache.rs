//! Data cache subsystem for caching read/write plain data.
use super::CacheState;
use crate::prelude::*;
use crate::util::RangeQueryCtx;
use crate::{Checkpoint, LsmTree, Record};

use std::collections::HashMap;
use std::future::Future;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A cache for data. It consists of a buffer pool to manage
/// multi segment buffers.
pub struct DataCache {
    buffer_pool: Vec<Arc<SegmentBuffer>>,
    curr_idx: AtomicUsize,
    capacity: usize,
    is_init: AtomicBool,
    arw_lock: AsyncRwLock<()>,
}

/// Segment buffer. It caches and manages plain data blocks of one segment.
pub struct SegmentBuffer {
    plain_data_blocks: RwLock<HashMap<Lba, DataBlock>>,
    state: Mutex<CacheState>,
    capacity: usize,
    disk: DiskView,
    pollee: Pollee,
    checkpoint: Arc<Checkpoint>,
    lsm_tree: Arc<LsmTree>,
}

impl DataCache {
    /// Initialize a `DataCache` given a capacity of pool.
    pub fn new(
        pool_capacity: usize,
        disk: DiskView,
        checkpoint: Arc<Checkpoint>,
        lsm_tree: Arc<LsmTree>,
    ) -> Self {
        Self {
            buffer_pool: {
                let mut pool = Vec::with_capacity(pool_capacity);
                for _ in 0..pool_capacity {
                    pool.push(Arc::new(SegmentBuffer::new(
                        SEGMENT_BUFFER_CAPACITY,
                        disk.clone(),
                        checkpoint.clone(),
                        lsm_tree.clone(),
                    )))
                }
                pool
            },
            curr_idx: AtomicUsize::new(0),
            capacity: pool_capacity,
            is_init: AtomicBool::new(true),
            arw_lock: AsyncRwLock::new(()),
        }
    }

    /// Insert a block buffer to cache.
    pub async fn insert(&self, lba: Lba, buf: &[u8]) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;

        self.insert_block(lba, buf).await?;

        drop(aw_lock);
        Ok(())
    }

    /// Search the data block from a target lba.
    pub async fn search(&self, target_lba: Lba, buf: &mut [u8]) -> Result<usize> {
        let ar_lock = self.arw_lock.read().await;

        let cap = self.capacity;
        let cur = self.curr_idx.load(Ordering::Relaxed);

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
        let cur = self.curr_idx.load(Ordering::Relaxed);

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
    async fn insert_block(&self, lba: Lba, buf: &[u8]) -> Result<()> {
        let curr_idx = self.curr_idx.load(Ordering::Relaxed);
        let curr_buf = &self.buffer_pool[curr_idx];

        let is_full = curr_buf.insert(lba, buf).await?;

        // If the segment buffer is full, shift current index of pool
        // and start a writeback task
        if is_full {
            self.curr_idx
                .store((curr_idx + 1) % self.capacity, Ordering::Relaxed);

            curr_buf.clear_events(SegBufEvents::Writeback);

            let curr_buf = curr_buf.clone();
            let prev_buf = if curr_idx == 0 {
                self.buffer_pool.last().unwrap().clone()
            } else {
                self.buffer_pool[curr_idx - 1].clone()
            };
            let is_init = if self.is_init.load(Ordering::Relaxed) {
                self.is_init.store(false, Ordering::Relaxed);
                true
            } else {
                false
            };
            // Background writeback
            #[cfg(feature = "sgx")]
            async_rt::task::spawn(async move {
                curr_buf
                    .encrypt_and_persist(prev_buf.wait_indexing(), is_init)
                    .await
                    .unwrap();
            });
            // Foreground writeback (Test-purpose)
            #[cfg(not(feature = "sgx"))]
            curr_buf
                .encrypt_and_persist(prev_buf.wait_indexing(), is_init)
                .await?;
        }
        Ok(())
    }

    /// Search or insert a data block to cache.
    /// Used for segment cleaning, must be atomic.
    pub async fn search_or_insert(&self, lba: Lba, buf: &[u8]) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;

        for seg_buf in &self.buffer_pool {
            if seg_buf.check_newer(lba).await {
                // Newer data are written, no need to migrate old one
                return Ok(());
            }
        }

        // No newer data found, migrate this block
        self.insert_block(lba, buf).await?;

        drop(aw_lock);
        Ok(())
    }

    pub async fn persist(&self) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;

        for seg_buf in &self.buffer_pool {
            seg_buf.wait_writeback().await?;

            // Flush partial filled segment buffer
            seg_buf.set_state(CacheState::Full);
            seg_buf.encrypt_and_persist(async { Ok(()) }, false).await?;
        }

        drop(aw_lock);
        Ok(())
    }
}

impl SegmentBuffer {
    /// Initialize a `SegmentBuffer` given a capacity.
    pub fn new(
        capacity: usize,
        disk: DiskView,
        checkpoint: Arc<Checkpoint>,
        lsm_tree: Arc<LsmTree>,
    ) -> Self {
        Self {
            plain_data_blocks: RwLock::new(HashMap::new()),
            state: Mutex::new(CacheState::Vacant),
            capacity,
            disk,
            pollee: Pollee::new(Events::empty()),
            checkpoint,
            lsm_tree,
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
                    self.wait_events(SegBufEvents::Writeback).await?;
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

    /// Check if segment buffer already contains written block of target lba.
    async fn check_newer(&self, target_lba: Lba) -> bool {
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
                if let Some(record) = self.lsm_tree.search(target_lba).await {
                    if !record.is_negative() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Allocate and a segment to current segment buffer.
    fn try_alloc_segment(&self) -> Result<Hba> {
        // Allocate a segment from data `SVT`
        let picked_seg = self.checkpoint.data_svt().write().pick_avail_seg()?;

        // Update the segment info in `DST`
        self.checkpoint.dst().write().validate_or_insert(picked_seg);

        Ok(picked_seg)
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
                    self.wait_events(SegBufEvents::Writeback).await?;
                    state = self.state.lock();
                }
            }
        }

        drop(state);
        Ok(())
    }

    async fn wait_indexing(&self) -> Result<()> {
        {
            let state = self.state.lock();
            match *state {
                CacheState::Vacant | CacheState::Clearing => {}
                CacheState::Full | CacheState::Flushing => {
                    drop(state);
                    self.wait_events(SegBufEvents::Indexing).await?;
                }
            }
        }
        self.clear_events(SegBufEvents::Indexing);
        Ok(())
    }

    async fn wait_events(&self, events: SegBufEvents) -> Result<()> {
        let poller = Poller::new();
        if self.pollee.poll(events.events(), Some(&poller)).is_empty() {
            poller.wait().await?;
        }
        Ok(())
    }

    fn notify_events(&self, events: SegBufEvents) {
        self.pollee.add_events(events.events());
    }

    fn clear_events(&self, events: SegBufEvents) {
        self.pollee.del_events(events.events());
    }

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
    async fn encrypt_and_persist(
        &self,
        wait_prebuf_indexing: impl Future<Output = Result<()>>,
        is_init: bool,
    ) -> Result<()> {
        // Check buffer state
        let mut state = self.state.lock();
        assert!(*state == CacheState::Full);
        *state = CacheState::Flushing;
        drop(state);

        let num_persist = self.plain_data_blocks.read().len();
        debug_assert!(num_persist <= self.capacity);
        let is_partial_persist: bool = {
            if num_persist == self.capacity {
                false
            } else if num_persist == 0 {
                *self.state.lock() = CacheState::Vacant;
                return Ok(());
            } else {
                true
            }
        };
        let mut left_blocks = vec![];

        // Allocate and collect blocks
        let allocated_blocks = {
            if let Ok(seg_addr) = self.try_alloc_segment() {
                if is_partial_persist {
                    left_blocks.extend_from_slice(
                        &DiskRangeIter::new(&HbaRange::new(
                            seg_addr + num_persist as _..seg_addr + self.capacity as _,
                        ))
                        .collect::<Vec<_>>(),
                    )
                }
                DiskRangeIter::new(&HbaRange::new(seg_addr..seg_addr + num_persist as _))
                    .collect::<Vec<_>>()
            } else {
                // Enable threaded logging when there is no free segment
                self.checkpoint
                    .dst()
                    .write()
                    .alloc_blocks(num_persist)
                    .unwrap()
            }
        };

        let data_blocks = self.plain_data_blocks.read();
        // Collect data and sort each block by lba
        let mut sorted_pbs = data_blocks.iter().collect::<Vec<_>>();
        sorted_pbs.sort_by_key(|kv| kv.0);
        // Encrypt each block
        let mut sorted_cbs = sorted_pbs
            .iter()
            .enumerate()
            .map(|(idx, (&lba, data_block))| {
                (
                    lba,
                    DefaultCryptor::encrypt_block_aead(
                        data_block.as_slice(),
                        &self.checkpoint.key_table().fetch_key(allocated_blocks[idx]),
                    ),
                    allocated_blocks[idx],
                )
            })
            .collect::<Vec<_>>();
        drop(data_blocks);

        let mut new_records = Vec::with_capacity(sorted_cbs.len());
        for sub_cbs in sorted_cbs.group_by_mut(|(_, _, hba1), (_, _, hba2)| {
            hba2.to_raw().saturating_sub(hba1.to_raw()) == 1
        }) {
            let mut wbufs = Vec::with_capacity(sub_cbs.len());
            let start_addr = sub_cbs.first().unwrap().2;

            for (lba, cipher_block, hba) in sub_cbs.iter_mut() {
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
                new_records.push(Record::new(*lba, *hba, cipher_block.cipher_meta().clone()));
            }

            // Write back cipher blocks to disk
            self.write_consecutive_blocks(start_addr, wbufs).await?;
        }

        // Update `RIT`
        for new_record in new_records.iter() {
            self.checkpoint
                .rit()
                .write()
                .await
                .insert(new_record.hba(), new_record.lba())
                .await?;
        }

        // Update index only when data are successfully persisted
        // and previous buffer finished indexing
        if !is_init {
            wait_prebuf_indexing.await?;
        }
        for new_record in new_records {
            self.lsm_tree.insert(new_record.lba(), new_record).await?;
        }
        self.notify_events(SegBufEvents::Indexing);

        // Update not used blocks info within a segment in `DST`
        // Used in partial persist while receiving a sync request
        if is_partial_persist && !left_blocks.is_empty() {
            self.checkpoint
                .dst()
                .write()
                .update_validity(&left_blocks, false);
        }

        // Update state and clear buffer
        *self.state.lock() = CacheState::Clearing;
        self.plain_data_blocks.write().clear();

        // Update state and notify a event
        *self.state.lock() = CacheState::Vacant;
        self.notify_events(SegBufEvents::Writeback);
        Ok(())
    }

    async fn write_consecutive_blocks(&self, addr: Hba, write_bufs: Vec<BlockBuf>) -> Result<()> {
        debug_assert!(write_bufs.len() <= self.capacity);

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

enum SegBufEvents {
    Writeback,
    Indexing,
}

impl SegBufEvents {
    fn events(&self) -> Events {
        match self {
            SegBufEvents::Writeback => Events::OUT,
            SegBufEvents::Indexing => Events::IN,
        }
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
