use std::collections::HashMap;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutex as Mutex, SgxMutexGuard as MutexGuard};

mod page;
mod page_entry;
mod page_handle;
mod page_lru_list;
mod page_state;

pub use self::page::Page;
pub use self::page_handle::PageHandle;
pub use self::page_state::PageState;

use self::page_entry::{PageEntry, PageEntryInner};
use self::page_lru_list::PageLruList;

/// Page cache.
pub struct PageCache {
    capacity: usize,
    num_allocated: AtomicUsize,
    map: Mutex<HashMap<(i32, usize), PageEntry>>,
    lru_lists: [Mutex<PageLruList>; 3],
}

pub trait AsFd {
    fn as_fd(&self) -> i32;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum LruListName {
    // For any entry : &PageEntry in &lru_lists[LruListName::Unused], we have
    //      entry.state == PageState::Uninit && PageEntry::refcnt(entry) == 1
    Unused = 0,
    // For any entry : &PageEntry in &lru_lists[LruListName::Evictable], we have
    //      entry.state == PageState::UpToDate && PageEntry::refcnt(entry) == 2
    Evictable = 1,
    // For any entry : &PageEntry in &lru_lists[LruListName::Dirty], we have
    //      entry.state == PageState::Dirty (most likely, but not always)
    //   && PageEntry::refcnt(entry) > 2
    Dirty = 2,
}

impl PageCache {
    /// Create a page cache that can contain an specified number of pages at most.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0);
        let num_allocated = AtomicUsize::new(0);
        let map = Mutex::new(HashMap::new());
        let lru_lists = array_init::array_init(|_| Mutex::new(PageLruList::new()));
        Self {
            capacity,
            num_allocated,
            map,
            lru_lists,
        }
    }

    /// Acquire a page handle for the given fd and offset.
    ///
    /// The returned page handle may be fetched from the cache or newly created.
    pub fn acquire<F>(&self, file: &Arc<F>, offset: usize) -> Option<PageHandle>
    where
        F: AsFd + Send + Sync + 'static,
    {
        debug_assert!(offset % Page::size() == 0);

        let key = (file.as_fd(), offset);
        let mut map = self.map.lock().unwrap();

        // Try to get an existing entry in the map.
        if let Some(existing_entry) = map.get(&key) {
            self.touch_lru_list(existing_entry);
            return Some(PageHandle::wrap(existing_entry.clone()));
        }

        // Try to create an new entry
        // First attempt: reuse an entry that is previously allocated, but currently not in use.
        let reusable_entry_opt = self.evict_from_lru_list(LruListName::Unused);
        let new_entry = if let Some(mut reusable_entry) = reusable_entry_opt {
            unsafe {
                debug_assert!(PageEntry::refcnt(&reusable_entry) == 1);
                reusable_entry.reset(file.clone(), offset);
            }
            reusable_entry
        }
        // Second attempt: allocate a new entry if the capacity won't be exceeded
        else if self.num_allocated.load(Ordering::Relaxed) < self.capacity {
            self.num_allocated.fetch_add(1, Ordering::Relaxed);
            PageEntry::new(file.clone(), offset)
        }
        // Last attempt: evict an entry from the evictable LRU list
        else {
            let evicted_entry_opt = self.evict_from_lru_list(LruListName::Evictable);
            let mut evicted_entry = match evicted_entry_opt {
                Some(evicted_entry) => evicted_entry,
                None => {
                    return None;
                }
            };
            map.remove(&evicted_entry.key());

            unsafe {
                debug_assert!(PageEntry::refcnt(&evicted_entry) == 1);
                evicted_entry.reset(file.clone(), offset);
                *evicted_entry.state() = PageState::Uninit;
            }
            evicted_entry
        };
        map.insert(key, new_entry.clone()).unwrap_none();

        Some(PageHandle::wrap(new_entry))
    }

    /// Release a page handle.
    pub fn release(&self, handle: PageHandle) {
        self.do_release(handle, false)
    }

    pub fn discard(&self, handle: PageHandle) {
        self.do_release(handle, true)
    }

    /// Evict some LRU dirty pages.
    ///
    /// Note that the results may contain false positives.
    pub fn evict_dirty_pages(&self, max_count: usize) -> Vec<PageHandle> {
        let mut lru_dirty_list = self.acquire_lru_list(LruListName::Dirty);
        let evicted: Vec<PageEntry> = lru_dirty_list.evict_nr(max_count);
        for entry in &evicted {
            entry.set_list_name(None);
        }
        unsafe {
            // This transmute is ok because PageEntry and PageHandle have
            // exactly the same memory layout.
            std::mem::transmute(evicted)
        }
    }

    /// Evict some LRU dirty pages of a file.
    ///
    /// Note that the results may contain false positives.
    pub fn evict_dirty_pages_by_file<F: AsFd>(
        &self,
        file: &F,
        max_count: usize,
    ) -> Vec<PageHandle> {
        self.evict_dirty_pages_by_fd(file.as_fd(), max_count)
    }

    pub fn evict_dirty_pages_by_fd(&self, fd: i32, max_count: usize) -> Vec<PageHandle> {
        let mut lru_dirty_list = self.acquire_lru_list(LruListName::Dirty);
        let cond = |entry: &PageEntryInner| entry.fd() == fd;
        let evicted: Vec<PageEntry> = lru_dirty_list.evict_nr_with(max_count, cond);
        for entry in &evicted {
            entry.set_list_name(None);
        }
        unsafe {
            // This transmute is ok because PageEntry and PageHandle have
            // exactly the same memory layout.
            std::mem::transmute(evicted)
        }
    }

    pub fn num_dirty_pages(&self) -> usize {
        let lru_dirty_list = self.acquire_lru_list(LruListName::Dirty);
        lru_dirty_list.len()
    }

    fn do_release(&self, handle: PageHandle, is_discard: bool) {
        let entry = handle.unwrap();
        let mut map = self.map.lock().unwrap();

        let are_users_still_holding_handles = |entry: &PageEntry| {
            let internal_refcnt = if entry.list_name().is_some() {
                2 // 1 for lru_list + 1 for map
            } else {
                1 // 1 for lru_list
            };
            let user_refcnt = PageEntry::refcnt(entry) - internal_refcnt;
            user_refcnt > 1
        };

        let dst_list_name = {
            let mut state = entry.state();
            if are_users_still_holding_handles(&entry) {
                match *state {
                    PageState::Dirty => Some(LruListName::Dirty),
                    _ => None,
                }
            } else {
                // This is the right timing to "free" a page cache
                if is_discard {
                    *state = PageState::Uninit;
                }
                if *state == PageState::Uninit {
                    map.remove(&entry.key());
                }

                match *state {
                    PageState::Uninit => Some(LruListName::Unused),
                    PageState::UpToDate => Some(LruListName::Evictable),
                    PageState::Dirty => Some(LruListName::Dirty),
                    _ => None,
                }
            }
        };
        self.reinsert_to_lru_list(entry, dst_list_name);
    }

    fn reinsert_to_lru_list(&self, entry: PageEntry, dst_list_name: Option<LruListName>) {
        let src_list_name = entry.list_name();

        entry.set_list_name(dst_list_name);
        match (src_list_name, dst_list_name) {
            (None, None) => {
                // Do nothing
            }
            (None, Some(dst_list_name)) => {
                let mut dst_list = self.acquire_lru_list(dst_list_name);
                dst_list.insert(entry);
            }
            (Some(src_list_name), None) => {
                let mut src_list = self.acquire_lru_list(src_list_name);
                src_list.remove(&entry);
            }
            (Some(src_list_name), Some(dst_list_name)) => {
                if src_list_name == dst_list_name {
                    let mut src_dst_list = self.acquire_lru_list(src_list_name);
                    src_dst_list.touch(&entry);
                } else {
                    let mut src_list = self.acquire_lru_list(src_list_name);
                    src_list.remove(&entry);
                    drop(src_list);

                    let mut dst_list = self.acquire_lru_list(dst_list_name);
                    dst_list.insert(entry);
                    drop(dst_list);
                }
            }
        }
    }

    fn touch_lru_list(&self, entry: &PageEntry) {
        let lru_list_name = match entry.list_name() {
            Some(lru_list_name) => lru_list_name,
            None => {
                return;
            }
        };
        let mut lru_list = self.acquire_lru_list(lru_list_name);
        lru_list.touch(entry);
    }

    fn evict_from_lru_list(&self, name: LruListName) -> Option<PageEntry> {
        let evicted_entry_opt = self.acquire_lru_list(name).evict();
        let evicted_entry = match evicted_entry_opt {
            Some(evicted_entry) => evicted_entry,
            None => {
                return None;
            }
        };

        // Check some invariance
        debug_assert!(match name {
            LruListName::Unused => {
                PageEntry::refcnt(&evicted_entry) == 1
                    && *evicted_entry.state() == PageState::Uninit
            }
            LruListName::Evictable => {
                PageEntry::refcnt(&evicted_entry) == 2
                    && *evicted_entry.state() == PageState::UpToDate
            }
            LruListName::Dirty => {
                PageEntry::refcnt(&evicted_entry) > 2
            }
        });

        evicted_entry.set_list_name(None);
        Some(evicted_entry)
    }

    fn acquire_lru_list(&self, name: LruListName) -> MutexGuard<PageLruList> {
        self.lru_lists[name as usize].lock().unwrap()
    }
}

impl std::fmt::Debug for PageCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageCache")
            .field("capacity", &self.capacity)
            .field("num_allocated", &self.num_allocated.load(Ordering::Relaxed))
            .field("map", &self.map.lock().unwrap())
            .field(
                "lru_lists",
                &self
                    .lru_lists
                    .iter()
                    .map(|ll| ll.lock().unwrap())
                    .collect::<Vec<MutexGuard<PageLruList>>>(),
            )
            .finish()
    }
}

#[cfg(test)]
mod test {
    use self::helper::{release_pages, visit_page, File};
    use super::*;

    // Create a dummy file object.
    macro_rules! file {
        ($fd:expr) => {{
            let fd = $fd;
            Arc::new(File(fd))
        }};
    }

    #[test]
    fn create_an_uptodate_page() {
        let page_cache = PageCache::with_capacity(1);
        let page_key = (file!(0), 0);
        visit_page(&page_cache, &page_key, |state, _page_slice| {
            assert!(**state == PageState::Uninit);
            **state = PageState::UpToDate;
        });
        visit_page(&page_cache, &page_key, |state, _page_slice| {
            assert!(**state == PageState::UpToDate);
        });
    }

    #[test]
    fn write_and_read_a_page() {
        let page_cache = PageCache::with_capacity(1);
        let page_key = (file!(0), 0);
        visit_page(&page_cache, &page_key, |_state, page_slice| {
            page_slice.fill(0xab);
        });
        visit_page(&page_cache, &page_key, |_state, page_slice| {
            assert!(page_slice.iter().all(|b| *b == 0xab));
        });
    }

    #[test]
    fn garbage_collect_useless_pages() {
        let page_cache = PageCache::with_capacity(4);
        for i in (0..1000) {
            let file = file!(0);
            let offset = i * Page::size();
            visit_page(&page_cache, &(file, offset), |state, _page_slice| {
                assert!(**state == PageState::Uninit);
                // Both Uninit and UpToDate pages can be recycled. We set some pages as UpToDate.
                if offset % 3 == 0 {
                    **state = PageState::UpToDate;
                }
            });
        }
    }

    #[test]
    fn create_dirty_pages() {
        let page_cache = PageCache::with_capacity(2);
        assert!(page_cache.num_dirty_pages() == 0);

        let file = file!(0);
        let page_keys = [(file.clone(), 0), (file.clone(), Page::size())];
        // Mark page 0 dirty
        visit_page(&page_cache, &page_keys[0], |state, _page_slice| {
            **state = PageState::Dirty;
        });
        assert!(page_cache.num_dirty_pages() == 1);
        // Mark page 1 dirty
        visit_page(&page_cache, &page_keys[1], |state, _page_slice| {
            **state = PageState::Dirty;
        });
        assert!(page_cache.num_dirty_pages() == 2);
        // Clean up page 1
        visit_page(&page_cache, &page_keys[1], |state, _page_slice| {
            **state = PageState::UpToDate;
        });
        assert!(page_cache.num_dirty_pages() == 1);
        // Clean up page 0
        visit_page(&page_cache, &page_keys[0], |state, _page_slice| {
            **state = PageState::UpToDate;
        });
        assert!(page_cache.num_dirty_pages() == 0);
    }

    #[test]
    fn evict_lru_pages() {
        // The cache can contain at most two pages
        let page_cache = PageCache::with_capacity(2);

        let page_keys = [(file!(0), 0), (file!(1), 0), (file!(2), 0)];
        // Touch page 0
        visit_page(&page_cache, &page_keys[0], |state, _page_slice| {
            **state = PageState::UpToDate;
        });
        // Touch page 1
        visit_page(&page_cache, &page_keys[1], |state, _page_slice| {
            **state = PageState::UpToDate;
        });
        // Touch page 2, evicting page 0
        visit_page(&page_cache, &page_keys[2], |_state, _page_slice| {});
        // Revisit page 0, whose state must be Uninit, since it has been evicted.
        // By revisting page 0, we have now evicted page 1.
        visit_page(&page_cache, &page_keys[0], |state, _page_slice| {
            assert!(**state == PageState::Uninit);
        });
        // Revisit page 1, which must be still in the cache
        visit_page(&page_cache, &page_keys[1], |state, _page_slice| {
            assert!(**state == PageState::UpToDate);
        });
    }

    #[test]
    fn evict_dirty_pages() {
        let page_cache = PageCache::with_capacity(3);

        // Mark three pages as dirty
        let page_keys = [(file!(0), 0), (file!(1), 0), (file!(2), 0)];
        for page_key in &page_keys {
            visit_page(&page_cache, page_key, |state, _page_slice| {
                **state = PageState::Dirty;
            });
        }

        let evicted_pages0 = page_cache.evict_dirty_pages(1);
        assert!(page_cache.num_dirty_pages() == 2);
        assert!(evicted_pages0.len() == 1);
        assert!(evicted_pages0[0].key() == (0, 0));

        let evicted_pages1 = page_cache.evict_dirty_pages(2);
        assert!(page_cache.num_dirty_pages() == 0);
        assert!(evicted_pages1.len() == 2);
        assert!(evicted_pages1[0].key() == (1, 0));
        assert!(evicted_pages1[1].key() == (2, 0));

        release_pages(&page_cache, evicted_pages0.into_iter());
        release_pages(&page_cache, evicted_pages1.into_iter());
        assert!(page_cache.num_dirty_pages() == 3);
    }

    #[test]
    fn hold_multiple_handles() {
        let page_cache = PageCache::with_capacity(1);
        let file = file!(10);
        let key = 0;
        let handles = (0..10)
            .map(|_| page_cache.acquire(&file, key).unwrap())
            .collect::<Vec<PageHandle>>();
        release_pages(&page_cache, handles.into_iter());
    }

    #[test]
    fn fill_up_cache() {
        let page_cache = PageCache::with_capacity(1);
        let page_key = (file!(0), 0);
        visit_page(&page_cache, &page_key, |state, _page_slice| {
            **state = PageState::Dirty;
        });
        let another_page_key = (file!(1), Page::size());
        assert!(page_cache
            .acquire(&another_page_key.0, another_page_key.1)
            .is_none());
    }

    #[test]
    fn discard_page() {
        let page_cache = PageCache::with_capacity(1);
        let page_key = (file!(0), 0);
        visit_page(&page_cache, &page_key, |state, _page_slice| {
            **state = PageState::Dirty;
        });
        assert!(page_cache.num_dirty_pages() == 1);

        let page_handle = page_cache.acquire(&page_key.0, page_key.1).unwrap();
        page_cache.discard(page_handle);
        assert!(page_cache.num_dirty_pages() == 0);
    }

    #[test]
    fn downcast_file() {
        let page_cache = PageCache::with_capacity(1);
        let input_file = file!(1234);
        let offset = 0;
        let page_handle = page_cache.acquire(&input_file, offset).unwrap();
        let result_file = page_handle.file().clone().downcast::<File>().unwrap();
        assert!(input_file == result_file);
        page_cache.release(page_handle);
    }

    mod helper {
        use super::*;
        use std::sync::{Mutex, MutexGuard};

        #[derive(Debug, PartialEq, Eq)]
        pub struct File(pub i32);

        impl AsFd for File {
            fn as_fd(&self) -> i32 {
                self.0
            }
        }

        pub fn visit_page<F>(page_cache: &PageCache, page_key: &(Arc<File>, usize), visit_fn: F)
        where
            F: FnOnce(&mut MutexGuard<PageState>, &mut [u8]),
        {
            let page_handle = page_cache.acquire(&page_key.0, page_key.1).unwrap();
            let mut state = page_handle.state();
            let page_slice_mut = unsafe { page_handle.page().as_slice_mut() };

            visit_fn(&mut state, page_slice_mut);

            drop(state);
            page_cache.release(page_handle);
        }

        pub fn release_pages(page_cache: &PageCache, iter: impl Iterator<Item = PageHandle>) {
            iter.for_each(|page_handle| page_cache.release(page_handle));
        }
    }
}
