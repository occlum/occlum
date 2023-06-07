use crate::page_cache::PageCacheInner;
use crate::prelude::*;
use block_device::AnyMap;
use lazy_static::lazy_static;

use std::future::Future;
use std::marker::PhantomData;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Page evictor.
///
/// Page caches (`PageCache<K, A>`) using the same memory allocator
/// (`A: PageAlloc`) share a common page evictor, which flushes
/// dirty pages and evict pages for the page caches when
/// the memory allocator's free memory is low.
pub(crate) struct PageEvictor<K: PageKey, A: PageAlloc> {
    marker: PhantomData<(K, A)>,
}

impl<K: PageKey, A: PageAlloc> PageEvictor<K, A> {
    /// Register a page cache.
    ///
    /// This is called in the constructor of a page
    /// cache instance.
    pub fn register(page_cache: &PageCache<K, A>) {
        let evictor_task = Self::task_singleton();
        evictor_task.register(&page_cache.0);
    }

    /// Unregister a page cache.
    pub fn unregister(page_cache: &PageCache<K, A>) {
        let evictor_task = Self::task_singleton();
        evictor_task.unregister(&page_cache.0);
    }

    fn task_singleton() -> Arc<EvictorTaskInner<K, A>> {
        lazy_static! {
            static ref EVICTOR_TASKS: Mutex<AnyMap> = Mutex::new(AnyMap::new());
        }
        let mut tasks = EVICTOR_TASKS.lock();

        if let Some(task) = tasks.get::<EvictorTask<K, A>>() {
            task.0.clone()
        } else {
            let new_task = EvictorTask::<K, A>::new();
            tasks.insert(new_task.clone());
            new_task.0
        }
    }
}

#[derive(Clone)]
struct EvictorTask<K: PageKey, A: PageAlloc>(Arc<EvictorTaskInner<K, A>>);

impl<K: PageKey, A: PageAlloc> std::fmt::Debug for EvictorTask<K, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EvictorTask(...)")
    }
}

struct EvictorTaskInner<K: PageKey, A: PageAlloc> {
    caches: Mutex<Vec<Arc<PageCacheInner<K, A>>>>,
    evictor_wq: WaiterQueue,
    is_dropped: AtomicBool,
    marker: PhantomData<(K, A)>,
}

impl<K: PageKey, A: PageAlloc> EvictorTask<K, A> {
    pub fn new() -> Self {
        let new_self = { Self(Arc::new(EvictorTaskInner::new())) };

        let this = new_self.0.clone();
        A::register_low_memory_callback(move || {
            this.evictor_wq.wake_all();
        });

        let this = new_self.0.clone();
        async_rt::task::spawn(async move {
            this.task_main().await;
        });

        new_self
    }
}

impl<K: PageKey, A: PageAlloc> EvictorTaskInner<K, A> {
    pub fn new() -> Self {
        EvictorTaskInner {
            caches: Mutex::new(Vec::new()),
            evictor_wq: WaiterQueue::new(),
            is_dropped: AtomicBool::new(false),
            marker: PhantomData,
        }
    }

    pub fn register(&self, page_cache: &Arc<PageCacheInner<K, A>>) {
        let mut caches = self.caches.lock();
        caches.push(page_cache.clone());
    }

    pub fn unregister(&self, page_cache: &Arc<PageCacheInner<K, A>>) {
        let id = page_cache.id();
        let mut caches = self.caches.lock();
        caches.retain(|v| v.id() != id);
    }

    #[allow(unused)]
    async fn task_main(&self) {
        let mut waiter = Waiter::new();
        self.evictor_wq.enqueue(&mut waiter);
        while !self.is_dropped() {
            waiter.reset();

            while A::is_memory_low() {
                self.evict_pages().await;
            }

            // Wait until being notified
            waiter.wait().await;
        }
        self.evictor_wq.dequeue(&mut waiter);
    }

    async fn evict_pages(&self) {
        // Flush all page caches
        self.for_each_page_cache_async(async move |page_cache| {
            page_cache.flush().await;
        })
        .await;

        // Evict pages to free memory
        const BATCH_EVICT_SIZE: usize = 25_000;
        while A::is_memory_low() {
            let mut total_evicted = 0;
            // Evict all page caches
            self.for_each_page_cache(|page_cache| {
                total_evicted += page_cache.evict(BATCH_EVICT_SIZE);
            });
            trace!("[PageEvictor] memory low, total evicted: {}", total_evicted);
            if total_evicted == 0 {
                break;
            }
        }
    }

    async fn for_each_page_cache_async<F, Fut>(&self, f: F)
    where
        F: Fn(Arc<PageCacheInner<K, A>>) -> Fut,
        Fut: Future<Output = ()>,
    {
        if let Some(caches) = self.caches.try_lock() {
            if caches.len() > 0 {
                // TODO: Load balance between the page caches
                // so that pages are evenly evicted
                for i in 0..caches.len() {
                    f(caches[i].clone()).await;
                }
            }
            drop(caches);
        }
    }

    fn for_each_page_cache<F>(&self, mut f: F)
    where
        F: FnMut(&Arc<PageCacheInner<K, A>>),
    {
        let caches = self.caches.lock();
        if caches.len() > 0 {
            for i in 0..caches.len() {
                f(&caches[i]);
            }
        }
        drop(caches);
    }

    fn is_dropped(&self) -> bool {
        self.is_dropped.load(Ordering::Relaxed)
    }
}

impl<K: PageKey, A: PageAlloc> Drop for EvictorTaskInner<K, A> {
    fn drop(&mut self) {
        self.is_dropped.store(true, Ordering::Relaxed);
    }
}
