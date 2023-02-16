use crate::prelude::*;
use block_device::{
    Bid, BioReqBuilder, BioType, BlockBuf, BlockDevice, BlockDeviceAsFile, BlockRangeIter,
    BLOCK_SIZE,
};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

/// A virtual disk with a backing disk and a page cache.
///
/// Thanks to the page cache, accessing a disk through
/// `CachedDisk` is expected to be faster than
/// accessing the disk directly.
///
/// `CachedDisk` exhibits the write-back strategy: writes
/// are first cached in memory, and later flushed to the
/// backing disk. The flush is either triggered by an
/// explicit flush operation or performed by a background
/// flusher task.
///
/// The memory allocator for the page cache is specified
/// by the generic parameter `A` of `CachedDisk<A: PageAlloc>`.
pub struct CachedDisk<A: PageAlloc>(Arc<Inner<A>>);

struct Inner<A: PageAlloc> {
    disk: Arc<dyn BlockDevice>,
    cache: PageCache<Bid, A>,
    flusher_wq: WaiterQueue,
    // This read-write lock is used to control the concurrent
    // writers and flushers. A writer acquires the read lock,
    // permitting multiple writers, but no flusher. A flusher
    // acquires the write lock to forbid any other flushers
    // and writers. This policy is important to implement
    // the semantic of the flush operation correctly.
    arw_lock: AsyncRwLock<()>,
    // Whether CachedDisk is dropped
    is_dropped: AtomicBool,
}

impl PageKey for Bid {}

impl<A: PageAlloc> CachedDisk<A> {
    /// Create a new `CachedDisk`.
    ///
    /// Specify a backing disk which implements `BlockDevice`.
    pub fn new(disk: Arc<dyn BlockDevice>) -> Result<Self> {
        let flusher = Arc::new(CachedDiskFlusher::<A>::new());
        let cache = PageCache::new(flusher.clone());
        let flusher_wq = WaiterQueue::new();
        let arc_inner = Arc::new(Inner {
            disk,
            cache,
            flusher_wq,
            arw_lock: AsyncRwLock::new(()),
            is_dropped: AtomicBool::new(false),
        });
        let new_self = Self(arc_inner);

        flusher.set_disk(new_self.0.clone());
        new_self.spawn_flusher_task();

        Ok(new_self)
    }

    /// Spawn a flusher task.
    ///
    /// The task flusher dirty pages in the page cache periodically.
    /// This flusher is not to be confused with `PageCacheFlusher`,
    /// the latter of which flushes dirty pages and evict pages to
    /// release memory when the free memory is low.
    fn spawn_flusher_task(&self) {
        const AUTO_FLUSH_PERIOD: Duration = Duration::from_secs(5);
        let this = self.0.clone();
        // Spawn the flusher task
        async_rt::task::spawn(async move {
            let mut waiter = Waiter::new();
            this.flusher_wq.enqueue(&mut waiter);
            loop {
                // If CachedDisk is dropped, then the flusher task should exit
                if this.is_dropped.load(Ordering::Relaxed) {
                    break;
                }

                // Wait until being notified or timeout
                let mut timeout = AUTO_FLUSH_PERIOD;
                let _ = waiter.wait_timeout(Some(&mut timeout)).await;

                // Do flush
                let _ = this.flush().await;
            }
            this.flusher_wq.dequeue(&mut waiter);
        });
    }

    /// Write back cached blocks to the underlying block device.
    ///
    /// On success, return the number of flushed pages.
    pub async fn flush(&self) -> Result<usize> {
        self.0.flush().await
    }
}

#[async_trait]
impl<A: PageAlloc> BlockDeviceAsFile for CachedDisk<A> {
    fn total_bytes(&self) -> usize {
        self.0.disk.total_blocks() * BLOCK_SIZE
    }

    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.0.read(offset, buf).await
    }

    async fn write(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        self.0.write(offset, buf).await
    }

    async fn sync(&self) -> Result<()> {
        self.0.sync().await
    }

    async fn flush_blocks(&self, blocks: &[Bid]) -> Result<usize> {
        self.0.flush_pages(blocks).await
    }
}

impl<A: PageAlloc> Inner<A> {
    /// Read cache content from `offset` into the given buffer.
    ///
    /// The length of buffer and offset can be arbitrary.
    /// On success, return the number of read bytes.
    pub async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.check_rw_args(offset, buf);
        let block_range_iter = BlockRangeIter {
            begin: offset,
            end: offset + buf.len(),
            block_size: BLOCK_SIZE,
        };

        let mut read_len = 0;
        for range in block_range_iter {
            let read_buf = &mut buf[read_len..read_len + range.len()];
            read_len += self
                .read_one_page(range.block_id, read_buf, range.begin)
                .await?;
        }

        debug_assert!(read_len == buf.len());
        Ok(read_len)
    }

    /// Write buffer content into cache starting from `offset`.
    ///
    /// The length of buffer and offset can be arbitrary.
    /// On success, return the number of written bytes.
    pub async fn write(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        self.check_rw_args(offset, buf);
        let block_range_iter = BlockRangeIter {
            begin: offset,
            end: offset + buf.len(),
            block_size: BLOCK_SIZE,
        };

        let mut write_len = 0;
        for range in block_range_iter {
            let write_buf = &buf[write_len..write_len + range.len()];
            write_len += self
                .write_one_page(range.block_id, write_buf, range.begin)
                .await?;
        }

        debug_assert!(write_len == buf.len());
        Ok(write_len)
    }

    /// Check if the arguments for a read or write is valid.
    fn check_rw_args(&self, offset: usize, buf: &[u8]) {
        debug_assert!(
            offset + buf.len() <= self.disk.total_blocks() * BLOCK_SIZE,
            "read/write length exceeds total blocks limit"
        );
    }

    /// Read a single page content from `offset` into the given buffer.
    async fn read_one_page(&self, bid: Bid, buf: &mut [u8], offset: usize) -> Result<usize> {
        debug_assert!(buf.len() + offset <= BLOCK_SIZE);

        let page_handle = self.acquire_page(bid).await?;
        let mut page_guard = page_handle.lock();

        // Ensure the page is ready for read
        loop {
            match page_guard.state() {
                // The page is ready for read
                PageState::UpToDate | PageState::Dirty | PageState::Flushing => {
                    break;
                }
                // The page is not initialized. So we need to
                // read it from the disk.
                PageState::Uninit => {
                    page_guard.set_state(PageState::Fetching);
                    Self::clear_page_events(&page_handle);

                    let page_ptr = page_guard.as_slice_mut();
                    let page_buf = unsafe {
                        std::slice::from_raw_parts_mut(page_ptr.as_mut_ptr(), BLOCK_SIZE)
                    };
                    drop(page_guard);

                    // Read one block from disk to current page
                    self.read_block(bid, page_buf).await?;

                    page_guard = page_handle.lock();
                    debug_assert!(page_guard.state() == PageState::Fetching);
                    page_guard.set_state(PageState::UpToDate);
                    Self::notify_page_events(&page_handle, Events::IN);
                    break;
                }
                // The page is being fetched. We just try again
                // later to see if it is ready.
                PageState::Fetching => {
                    drop(page_guard);
                    Self::wait_page_events(&page_handle, Events::IN).await;
                    page_guard = page_handle.lock();
                }
            }
        }

        let read_len = buf.len();
        let src_buf = page_guard.as_slice();
        buf.copy_from_slice(&src_buf[offset..offset + read_len]);

        drop(page_guard);
        self.cache.release(page_handle);
        Ok(read_len)
    }

    /// Write a single page content from `offset` into the given buffer.
    async fn write_one_page(&self, bid: Bid, buf: &[u8], offset: usize) -> Result<usize> {
        debug_assert!(buf.len() + offset <= BLOCK_SIZE);

        let page_handle = self.acquire_page(bid).await?;
        let ar_lock = self.arw_lock.read().await;
        let mut page_guard = page_handle.lock();

        // Ensure the page is ready for write
        loop {
            match page_guard.state() {
                PageState::Uninit => {
                    // Read latest content of current page from disk before write.
                    // Only occur in partial writes.
                    if buf.len() < BLOCK_SIZE {
                        page_guard.set_state(PageState::Fetching);
                        Self::clear_page_events(&page_handle);

                        let page_ptr = page_guard.as_slice_mut();
                        let page_buf = unsafe {
                            std::slice::from_raw_parts_mut(page_ptr.as_mut_ptr(), BLOCK_SIZE)
                        };
                        drop(page_guard);

                        self.read_block(bid, page_buf).await?;

                        page_guard = page_handle.lock();
                        debug_assert!(page_guard.state() == PageState::Fetching);
                        page_guard.set_state(PageState::UpToDate);
                        Self::notify_page_events(&page_handle, Events::IN);
                    }
                    break;
                }
                // The page is ready for write
                PageState::UpToDate | PageState::Dirty => {
                    break;
                }
                // The page is being fetched. We just try again
                // later to see if it is ready.
                PageState::Fetching | PageState::Flushing => {
                    drop(page_guard);
                    Self::wait_page_events(&page_handle, Events::IN | Events::OUT).await;
                    page_guard = page_handle.lock();
                }
            }
        }

        let write_len = buf.len();
        let dst_buf = page_guard.as_slice_mut();
        dst_buf[offset..offset + write_len].copy_from_slice(buf);
        page_guard.set_state(PageState::Dirty);

        drop(page_guard);
        self.cache.release(page_handle);
        drop(ar_lock);
        Ok(write_len)
    }

    /// Write back to block device block-by-block.
    ///
    /// Currently we use `flush()` (write back to disk in batches) rather than `flush_by_block()`
    /// for better I/O performance.
    #[allow(dead_code)]
    pub async fn flush_by_block(&self) -> Result<usize> {
        let mut total_pages = 0;
        let aw_lock = self.arw_lock.write().await;

        let mut flush_pages = Vec::with_capacity(128);
        const MAX_BATCH_SIZE: usize = 2048;
        loop {
            flush_pages.clear();
            let num_pages = self
                .cache
                .pop_dirty_to_flush(&mut flush_pages, MAX_BATCH_SIZE);
            if num_pages == 0 {
                break;
            }

            for page_handle in &flush_pages {
                let page_guard = page_handle.lock();
                debug_assert!(page_guard.state() == PageState::Flushing);
                Self::clear_page_events(&page_handle);

                let bid = page_handle.key();
                let page_ptr = page_guard.as_slice();
                let page_buf = unsafe { std::slice::from_raw_parts(page_ptr.as_ptr(), BLOCK_SIZE) };
                drop(page_guard);

                self.write_block(&bid, page_buf).await?;

                let mut page_guard = page_handle.lock();
                debug_assert!(page_guard.state() == PageState::Flushing);
                page_guard.set_state(PageState::UpToDate);
                Self::notify_page_events(&page_handle, Events::OUT);
                drop(page_guard);
            }

            total_pages += num_pages;
        }

        drop(aw_lock);
        // At this point, we can be certain that all writes
        // have been written back to the disk because
        // 1) There are no concurrent writers;
        // 2) There are no concurrent flushers;
        // 3) All dirty pages have been cleared.
        trace!("[CachedDisk] flush pages: {}", total_pages);
        Ok(total_pages)
    }

    /// Write back cached pages to block device in consecutive batches.
    ///
    /// On success, return the number of flushed pages.
    pub async fn flush(&self) -> Result<usize> {
        let mut total_pages = 0;
        let aw_lock = self.arw_lock.write().await;

        let mut flush_pages = Vec::with_capacity(128);
        const MAX_BATCH_SIZE: usize = 1024;
        loop {
            flush_pages.clear();
            let num_pages = self
                .cache
                .pop_dirty_to_flush(&mut flush_pages, MAX_BATCH_SIZE);
            if num_pages == 0 {
                break;
            }

            for page_handles in PageCache::group_consecutive_pages(&flush_pages) {
                let mut bufs = Vec::with_capacity(page_handles.len());
                for page_handle in page_handles {
                    let page_guard = page_handle.lock();
                    debug_assert!(page_guard.state() == PageState::Flushing);
                    Self::clear_page_events(&page_handle);

                    let page_ptr = page_guard.as_ptr();
                    drop(page_guard);
                    bufs.push(unsafe { BlockBuf::from_raw_parts(page_ptr, BLOCK_SIZE) });
                }

                let first_block_addr: Bid = page_handles[0].key();
                self.write_consecutive_blocks(first_block_addr, bufs)
                    .await?;

                for page_handle in page_handles {
                    let mut page_guard = page_handle.lock();
                    debug_assert!(page_guard.state() == PageState::Flushing);
                    page_guard.set_state(PageState::UpToDate);
                    Self::notify_page_events(&page_handle, Events::OUT);
                    drop(page_guard);
                }
            }

            total_pages += num_pages;
        }

        drop(aw_lock);
        // At this point, we can be certain that all writes
        // have been written back to the disk because
        // 1) There are no concurrent writers;
        // 2) There are no concurrent flushers;
        // 3) All dirty pages have been cleared.
        trace!("[CachedDisk] flush pages: {}", total_pages);
        Ok(total_pages)
    }

    /// Write back specified pages to block device, given an array of block IDs.
    pub async fn flush_pages(&self, pages: &[Bid]) -> Result<usize> {
        let mut total_pages = 0;

        for bid in pages {
            // If current page is not in the cache,
            // a new page with uninit state is returned.
            let page_handle = self.acquire_page(*bid).await?;
            let aw_lock = self.arw_lock.write().await;
            let mut page_guard = page_handle.lock();

            if page_guard.state() == PageState::Dirty {
                page_guard.set_state(PageState::Flushing);
                Self::clear_page_events(&page_handle);

                let page_ptr = page_guard.as_slice();
                let page_buf = unsafe { std::slice::from_raw_parts(page_ptr.as_ptr(), BLOCK_SIZE) };
                drop(page_guard);

                self.write_block(&bid, page_buf).await?;

                let mut page_guard = page_handle.lock();
                debug_assert!(page_guard.state() == PageState::Flushing);
                page_guard.set_state(PageState::UpToDate);
                Self::notify_page_events(&page_handle, Events::OUT);
                drop(page_guard);

                self.cache.release(page_handle);
                total_pages += 1;
            }
            drop(aw_lock)
        }

        trace!("[CachedDisk] flush specified pages: {}", total_pages);
        Ok(total_pages)
    }

    /// Write back all changes to block device then flush
    /// the underlying disk to ensure persistency.
    pub async fn sync(&self) -> Result<()> {
        self.flush().await?;
        self.disk.sync().await?;
        Ok(())
    }

    /// Acquire one page from page cache.
    /// Poll the readiness events on page cache if failed.
    async fn acquire_page(&self, block_id: Bid) -> Result<PageHandle<Bid, A>> {
        loop {
            if let Some(page_handle) = self.cache.acquire(block_id) {
                break Ok(page_handle);
            }

            let poller = Poller::new();
            let events = self.cache.poll(Some(&poller));
            if !events.is_empty() {
                continue;
            }
            poller.wait().await?;
        }
    }

    async fn read_block(&self, block_id: Bid, buf: &mut [u8]) -> Result<usize> {
        self.disk.read(block_id.to_offset(), buf).await
    }

    async fn write_block(&self, block_id: &Bid, buf: &[u8]) -> Result<usize> {
        self.disk.write(block_id.to_offset(), buf).await
    }

    async fn write_consecutive_blocks(&self, addr: Bid, write_bufs: Vec<BlockBuf>) -> Result<()> {
        let req = BioReqBuilder::new(BioType::Write)
            .addr(addr)
            .bufs(write_bufs)
            .build();
        let submission = self.disk.submit(Arc::new(req));
        let req = submission.complete().await;
        let res = req.response().unwrap();

        if let Err(e) = res {
            return Err(errno!(e.errno(), "write on a block device failed"));
        }
        Ok(())
    }

    fn clear_page_events(page_handle: &PageHandle<Bid, A>) {
        page_handle.pollee().reset_events();
    }

    fn notify_page_events(page_handle: &PageHandle<Bid, A>, events: Events) {
        page_handle.pollee().add_events(events);
    }

    #[allow(unused)]
    async fn wait_page_events(page_handle: &PageHandle<Bid, A>, events: Events) {
        let poller = Poller::new();
        if page_handle.pollee().poll(events, Some(&poller)).is_empty() {
            poller.wait().await;
        }
    }
}

impl<A: PageAlloc> Drop for CachedDisk<A> {
    fn drop(&mut self) {
        self.0.is_dropped.store(true, Ordering::Relaxed);
        self.0.flusher_wq.wake_all();
    }
}

struct CachedDiskFlusher<A: PageAlloc> {
    // this_opt => CachedDisk
    this_opt: Mutex<Option<Arc<Inner<A>>>>,
}

impl<A: PageAlloc> CachedDiskFlusher<A> {
    pub fn new() -> Self {
        Self {
            this_opt: Mutex::new(None),
        }
    }

    pub fn set_disk(&self, this: Arc<Inner<A>>) {
        *self.this_opt.lock() = Some(this);
    }

    fn this_opt(&self) -> Option<Arc<Inner<A>>> {
        self.this_opt.lock().clone()
    }
}

#[async_trait]
impl<A: PageAlloc> PageCacheFlusher for CachedDiskFlusher<A> {
    async fn flush(&self) -> Result<usize> {
        if let Some(this) = self.this_opt() {
            return this.flush().await;
        }
        Ok(0)
    }
}
