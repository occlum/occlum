//! PageCache tests
use async_rt::wait::Waiter;
use async_trait::async_trait;
use block_device::BLOCK_SIZE;
use errno::prelude::*;
use page_cache::*;

use std::sync::Arc;
use std::time::Duration;

type PageId = u64;
const MB: usize = 1024 * 1024;

macro_rules! new_page_cache_for_tests {
    ($cache_size:expr) => {{
        /// A flusher for page cache tests
        struct SimpleFlusher;
        #[async_trait]
        impl PageCacheFlusher for SimpleFlusher {
            async fn flush(&self) -> Result<usize> {
                Ok(0)
            }
        }

        let flusher = Arc::new(SimpleFlusher);

        // `MyPageAlloc` is a test-purpose fixed-size allocator.
        impl_fixed_size_page_alloc! { MyPageAlloc, $cache_size }

        PageCache::<PageId, MyPageAlloc>::new(flusher)
    }};
}

#[test]
fn page_cache_acquire_release() {
    let cache = new_page_cache_for_tests!(5 * MB);
    let key: PageId = 125;
    let content = [5u8; BLOCK_SIZE];

    let page_handle = cache.acquire(key).unwrap();
    let mut page_guard = page_handle.lock();
    assert_eq!(page_guard.state(), PageState::Uninit);
    page_guard.set_state(PageState::Dirty);

    // Write a page
    page_guard.as_slice_mut().copy_from_slice(&content);
    drop(page_guard);
    cache.release(page_handle);
    assert_eq!(cache.size(), 1);

    let page_handle = cache.acquire(key).unwrap();
    assert_eq!(page_handle.key(), key);
    let page_guard = page_handle.lock();
    assert_eq!(page_guard.state(), PageState::Dirty);

    // Read a page
    let read_content = page_guard.as_slice();
    assert_eq!(read_content, content);
    drop(page_guard);
    cache.release(page_handle);
    assert_eq!(cache.size(), 1);
}

#[test]
fn page_cache_pop_dirty_to_flush() {
    let cache = new_page_cache_for_tests!(5 * MB);
    let key: PageId = 125;

    let page_handle = cache.acquire(key).unwrap();
    let mut page_guard = page_handle.lock();
    page_guard.set_state(PageState::Dirty);
    drop(page_guard);
    cache.release(page_handle);

    let mut dirty = Vec::with_capacity(128);
    let dirty_num = cache.pop_dirty_to_flush(&mut dirty, 128);
    assert_eq!(dirty_num, 1);

    let page_handle = cache.acquire(key).unwrap();
    let mut page_guard = page_handle.lock();
    assert_eq!(page_guard.state(), PageState::Flushing);
    page_guard.set_state(PageState::UpToDate);
    drop(page_guard);
    cache.release(page_handle);

    let mut dirty = Vec::with_capacity(128);
    let dirty_num = cache.pop_dirty_to_flush(&mut dirty, 128);
    assert_eq!(dirty_num, 0);
}

#[test]
fn page_cache_group_consecutive_pages() {
    let cache = new_page_cache_for_tests!(5 * MB);
    let keys = vec![3 as PageId, 8, 7, 0, 2, 9, 5];
    let consecutive_keys = vec![vec![0 as PageId], vec![2, 3], vec![5], vec![7, 8, 9]];

    for key in &keys {
        let page_handle = cache.acquire(*key).unwrap();
        let mut page_guard = page_handle.lock();
        page_guard.set_state(PageState::Dirty);
        drop(page_guard);
        cache.release(page_handle);
    }

    let mut dirty_pages = Vec::with_capacity(128);
    let flush_num = cache.pop_dirty_to_flush(&mut dirty_pages, 128);

    let v: Vec<Vec<PageId>> = PageCache::group_consecutive_pages(&dirty_pages)
        .map(|page_handles| {
            page_handles
                .iter()
                .map(|page_handle| page_handle.key())
                .collect()
        })
        .collect();

    assert_eq!(v, consecutive_keys);
    assert_eq!(flush_num, keys.len());
}

#[test]
fn page_cache_evictor_task() -> Result<()> {
    async_rt::task::block_on(async move {
        let cache = new_page_cache_for_tests!(100 * BLOCK_SIZE);
        const CAPACITY: usize = 125;
        for key in 0..CAPACITY {
            if let Some(page_handle) = cache.acquire(key as _) {
                cache.release(page_handle);
            } else {
                break;
            }
        }

        let waiter = Waiter::new();
        let _ = waiter.wait_timeout(Some(&mut Duration::from_secs(5))).await;

        // Pages being evicted during out-limit acquire
        assert!(cache.size() < CAPACITY);
        Ok(())
    })
}
