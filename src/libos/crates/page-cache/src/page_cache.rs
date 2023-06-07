use crate::prelude::*;
use crate::PageEvictor;
use lru::LruCache;
use object_id::ObjectId;

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

pub type PageId = u64;
/// A trait to define domain of key for page cache.
pub trait PageKey: Into<PageId> + Copy + Send + Sync + Debug + 'static {}

/// Page cache.
pub struct PageCache<K: PageKey, A: PageAlloc>(pub(crate) Arc<PageCacheInner<K, A>>);

pub(crate) struct PageCacheInner<K: PageKey, A: PageAlloc> {
    id: ObjectId,
    flusher: Arc<dyn PageCacheFlusher>,
    cache: Mutex<LruCache<PageId, PageHandle<K, A>>>,
    dirty_set: Mutex<BTreeSet<PageId>>,
    pollee: Pollee,
    marker: PhantomData<(K, A)>,
}

/// Page cache flusher.
///
/// A page cache must be equipped with a user-given
/// flusher `F: PageCacheFlusher` so that when
/// the memory is low, the page cache mechanism
/// can automatically flush dirty pages and
/// subsequently evict pages.
///
/// This trait has only one method.
#[async_trait]
pub trait PageCacheFlusher: Send + Sync {
    /// Flush the dirty pages in a page cache.
    ///
    /// If success, then the return value is
    /// the number of dirty pages that are flushed.
    async fn flush(&self) -> Result<usize>;
}

impl<K: PageKey, A: PageAlloc> PageCache<K, A> {
    /// Create a new page cache.
    ///
    /// Specify a flusher which implements `PageCacheFlusher`.
    pub fn new(flusher: Arc<dyn PageCacheFlusher>) -> Self {
        info!("[PageCache] new");
        let new_self = Self(Arc::new(PageCacheInner::new(flusher)));
        PageEvictor::<K, A>::register(&new_self);
        new_self
    }

    /// Acquire the page that corresponds to the key.
    ///
    /// Return `None` if there are no available pages.
    /// In this case, the user can use the
    /// `poll` method to wait for the readiness of the
    /// page cache.
    pub fn acquire(&self, key: K) -> Option<PageHandle<K, A>> {
        let mut cache = self.0.cache.lock();
        // Cache hit
        if let Some(page_handle_incache) = cache.get(&key.into()) {
            return Some(page_handle_incache.clone());
        // Cache miss
        } else {
            self.0.pollee.reset_events();
            // Cache miss and a new page is allocated
            if let Some(page_handle) = PageHandle::new(key) {
                cache.put(key.into(), page_handle.clone());
                return Some(page_handle);
            }
            // Cache miss and no free space for new page
        }
        None
    }

    /// Release the page.
    ///
    /// All page handles obtained via the `acquire` method
    /// must be returned via the `release` method.
    pub fn release(&self, page_handle: PageHandle<K, A>) {
        // The dirty_set traces dirty pages in order
        let mut dirty_set = self.0.dirty_set.lock();
        let page_guard = page_handle.lock();
        // Update dirty_set when page_handle released
        if page_guard.state() == PageState::Dirty {
            dirty_set.insert(page_handle.key().into());
        } else {
            dirty_set.remove(&page_handle.key().into());
        }
    }

    /// Pop a number of dirty pages and switch their state to
    /// "Flushing".
    ///
    /// The handles of dirty pages are pushed into the given `Vec`.
    /// The dirty page IDs are in ascending order.
    /// The number of the dirty pages is returned.
    pub fn pop_dirty_to_flush(
        &self,
        dirty: &mut Vec<PageHandle<K, A>>,
        max_batch_size: usize,
    ) -> usize {
        debug_assert!(dirty.len() == 0);
        let cache = self.0.cache.lock();
        // The dirty_set traces dirty pages in order
        let mut dirty_set = self.0.dirty_set.lock();
        let mut flush_num = 0;

        while let Some(page_key) = dirty_set.pop_first() {
            if let Some(page_handle_incache) = cache.peek(&page_key) {
                let mut page_guard = page_handle_incache.lock();
                if page_guard.state() != PageState::Dirty {
                    continue;
                }

                page_guard.set_state(PageState::Flushing);
                dirty.push(page_handle_incache.clone());
                flush_num += 1;
                drop(page_guard);

                if flush_num >= max_batch_size {
                    break;
                }
            }
        }
        flush_num
    }

    /// Group a sorted slice of page handles into a series of slices,
    /// each of which contains pages of consecutive page IDs.
    ///
    /// The input slice is assumed to be sorted and contain no
    /// pages with the same IDs.
    pub fn group_consecutive_pages(
        page_handles: &[PageHandle<K, A>],
    ) -> impl Iterator<Item = &[PageHandle<K, A>]> {
        page_handles.group_by(|pa, pb| pb.key().into() - pa.key().into() <= 1)
    }

    pub fn size(&self) -> usize {
        let cache = self.0.cache.lock();
        cache.len()
    }

    /// Poll the readiness events on a page cache.
    ///
    /// The only interesting event is `Events::OUT`, which
    /// indicates that the page cache has evictable pages or
    /// the underlying page allocator has free space.
    ///
    /// This method is typically used after a failed attempt to
    /// acquire pages. In such situations, one needs to wait
    /// for the page cache to be ready for acquiring new pages.
    ///
    /// ```
    /// # async fn foo<A: PageAlloc>(page_cache: &PageCache<u64, A>) {
    /// let addr = 1234;
    /// let page = loop {
    ///     if Some(page) = page_cache.acquire(addr) {
    ///         break page;
    ///     }
    ///     
    ///     let mut poller = Poller::new();
    ///     let events = page_cache.poll(Events::OUT, Some(&mut poller));
    ///     if !events.is_empty() {
    ///         continue;
    ///     }
    ///
    ///     poller.wait().await.unwrap();
    /// }
    /// # }
    /// ```
    pub fn poll(&self, poller: Option<&Poller>) -> Events {
        self.0.poll(poller)
    }
}

impl<K: PageKey, A: PageAlloc> PageCacheInner<K, A> {
    /// Create a new page cache given a flusher.
    ///
    /// It mainly consists an unbounded lru-policy `cache` that leaves eviction to user
    /// and a `dirty_set` which tracks dirty page IDs.
    pub fn new(flusher: Arc<dyn PageCacheFlusher>) -> Self {
        PageCacheInner {
            id: ObjectId::new(),
            flusher,
            cache: Mutex::new(LruCache::unbounded()),
            dirty_set: Mutex::new(BTreeSet::new()),
            pollee: Pollee::new(Events::empty()),
            marker: PhantomData,
        }
    }

    /// Return the id of the page cache.
    pub const fn id(&self) -> ObjectId {
        self.id
    }

    /// Poll the readiness events on a page cache.
    pub fn poll(&self, poller: Option<&Poller>) -> Events {
        self.pollee.poll(Events::OUT, poller)
    }

    /// Evict a number of pages.
    ///
    /// The page cache uses a pseudo-LRU strategy to select
    /// the victim pages.
    pub(crate) fn evict(&self, max_evicted: usize) -> usize {
        let mut cache = self.cache.lock();
        let evict_total = max_evicted.min(cache.len());
        let mut evict_num = 0;
        for _ in 0..evict_total {
            if let Some((page_id, page_handle)) = cache.pop_lru() {
                let page_guard = page_handle.lock();
                // Make sure page state is evictable and no one holds current page handle
                if (page_guard.state() == PageState::UpToDate
                    || page_guard.state() == PageState::Uninit)
                    && Arc::strong_count(&page_handle.0) == 1
                {
                    drop(page_guard);
                    drop(page_handle);
                    evict_num += 1;
                } else {
                    drop(page_guard);
                    cache.put(page_id, page_handle.clone());
                }
            } else {
                break;
            }
        }

        self.pollee.add_events(Events::OUT);
        evict_num
    }

    /// Call flush method defined by `flusher: PageCacheFlusher`.
    pub(crate) async fn flush(&self) {
        let nflush = self.flusher.flush().await.unwrap();
        if nflush > 0 {
            trace!("[PageCache] flush pages: {}", nflush);
        }
    }
}

impl<K: PageKey, A: PageAlloc> Drop for PageCache<K, A> {
    fn drop(&mut self) {
        PageEvictor::<K, A>::unregister(&self);
    }
}

impl PageKey for PageId {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_cache_evict() {
        crate::impl_fixed_size_page_alloc! { MyPageAlloc, 1024 * 1024 }
        struct SimpleFlusher;
        #[async_trait]
        impl PageCacheFlusher for SimpleFlusher {
            async fn flush(&self) -> Result<usize> {
                Ok(0)
            }
        }

        let flusher = Arc::new(SimpleFlusher);
        let cache = PageCache::<PageId, MyPageAlloc>::new(flusher);

        const CAPACITY: usize = 15;
        // Create `UpToDate` pages
        for key in 0..5 {
            let page_handle = cache.acquire(key as _).unwrap();
            let mut page_guard = page_handle.lock();
            // Simulate reading page data from disk
            page_guard.set_state(PageState::Fetching);
            page_guard.set_state(PageState::UpToDate);
            drop(page_guard);
            cache.release(page_handle);
        }
        // Create `Uninit` pages
        for key in 5..10 {
            let page_handle = cache.acquire(key as _).unwrap();
            cache.release(page_handle);
        }
        // Create `Dirty` pages
        for key in 10..CAPACITY {
            let page_handle = cache.acquire(key as _).unwrap();
            let mut page_guard = page_handle.lock();
            // Simulate writing page data
            page_guard.set_state(PageState::Dirty);
            drop(page_guard);
            cache.release(page_handle);
        }
        assert_eq!(cache.size(), CAPACITY);

        // Make sure only `UpToDate` and `Uninit` pages are available to evict
        const EVICT_NUM: usize = 10;
        let evict_num = cache.0.evict(EVICT_NUM);
        assert_eq!(evict_num, EVICT_NUM);
        assert_eq!(cache.size(), CAPACITY - EVICT_NUM);

        let evict_num = cache.0.evict(10);
        assert_eq!(evict_num, 0);
        assert_eq!(cache.size(), CAPACITY - EVICT_NUM);
    }
}
