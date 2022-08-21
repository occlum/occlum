//! PageCache tests
use async_rt::wait::Waiter;
use async_trait::async_trait;
use block_device::BLOCK_SIZE;
use errno::prelude::*;
use page_cache::*;

use std::sync::Arc;
use std::time::Duration;

// MyPageAlloc is a test-purpose fixed-size allocator.
pub const MB: usize = 1024 * 1024;
impl_fixed_size_page_alloc! { MyPageAlloc, MB * 5 }

/// A flusher for page cache tests
struct SimpleFlusher;

#[async_trait]
impl PageCacheFlusher for SimpleFlusher {
    async fn flush(&self) -> Result<usize> {
        Ok(0)
    }
}

fn new_page_cache() -> PageCache<usize, MyPageAlloc> {
    let flusher = Arc::new(SimpleFlusher);
    PageCache::<usize, MyPageAlloc>::new(flusher)
}

fn read_page(page_handle: &PageHandle<usize, MyPageAlloc>) -> u8 {
    let page_guard = page_handle.lock();
    page_guard.as_slice()[0]
}

fn write_page(page_handle: &PageHandle<usize, MyPageAlloc>, content: u8) {
    let mut page_guard = page_handle.lock();
    const SIZE: usize = BLOCK_SIZE;
    page_guard.as_slice_mut().copy_from_slice(&[content; SIZE]);
}

#[test]
fn page_cache_acquire_release() {
    let cache = new_page_cache();
    let key: usize = 125;
    let content: u8 = 5;

    let page_handle = cache.acquire(key).unwrap();
    let mut page_guard = page_handle.lock();
    assert_eq!(page_guard.state(), PageState::Uninit);
    page_guard.set_state(PageState::Dirty);
    drop(page_guard);

    write_page(&page_handle, content);
    cache.release(page_handle);
    assert_eq!(cache.size(), 1);

    let page_handle = cache.acquire(key).unwrap();
    assert_eq!(page_handle.key(), key);
    let page_guard = page_handle.lock();
    assert_eq!(page_guard.state(), PageState::Dirty);
    drop(page_guard);

    let read_content = read_page(&page_handle);
    assert_eq!(read_content, content);
    cache.release(page_handle);
    assert_eq!(cache.size(), 1);
}

#[test]
fn page_cache_pop_dirty_to_flush() {
    let cache = new_page_cache();
    let key: usize = 125;

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
    let cache = new_page_cache();
    let keys = vec![3usize, 8, 7, 0, 2, 9, 5];
    let consecutive_keys = vec![vec![0usize], vec![2, 3], vec![5], vec![7, 8, 9]];

    for key in &keys {
        let page_handle = cache.acquire(*key).unwrap();
        let mut page_guard = page_handle.lock();
        page_guard.set_state(PageState::Dirty);
        drop(page_guard);
        cache.release(page_handle);
    }

    let mut dirty_pages = Vec::with_capacity(128);
    let flush_num = cache.pop_dirty_to_flush(&mut dirty_pages, 128);

    let v: Vec<Vec<usize>> = PageCache::group_consecutive_pages(&dirty_pages)
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
#[allow(unused)]
fn page_cache_evictor_task() -> Result<()> {
    async_rt::task::block_on(async move {
        impl_fixed_size_page_alloc! { TestPageAlloc, BLOCK_SIZE * 100 }
        let flusher = Arc::new(SimpleFlusher);
        let cache = PageCache::<usize, TestPageAlloc>::new(flusher);

        const CAPACITY: usize = 125;
        for key in 0..CAPACITY {
            if let Some(page_handle) = cache.acquire(key) {
                cache.release(page_handle);
            } else {
                break;
            }
        }

        let waiter = Waiter::new();
        waiter.wait_timeout(Some(&mut Duration::from_secs(1))).await;

        // Pages being evicted during out-limit acquire
        assert!(cache.size() < CAPACITY);
        Ok(())
    })
}
