use self::runtime::Runtime;
use async_file::*;
use std::time::Instant;

fn read_write_sample() {
    async_rt::task::block_on(async {
        let block_size = 4096;
        let file_size = 1024 * 1024 * 100; // 100 MB

        let file = {
            let path = format!("tmp.data").to_string();
            let flags = libc::O_RDWR | libc::O_CREAT | libc::O_TRUNC;
            let mode = libc::S_IRUSR | libc::S_IWUSR;
            AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
        };

        let vec = vec![0; block_size];
        let mut buf = vec.into_boxed_slice();
        let mut offset = 0;
        let start = Instant::now();
        while offset < file_size {
            let nbytes = file.write_at(offset, &buf[..]).await.unwrap();
            offset += nbytes;
        }
        let duration = start.elapsed();
        println!("async-file sequential write [file_size: {} bytes, block_size: {} bytes] costs time: {:?}", file_size, block_size, duration);

        offset = 0;
        let start = Instant::now();
        while offset < file_size {
            let nbytes = file.read_at(offset, &mut buf[..]).await.unwrap();
            offset += nbytes as usize;
        }
        let duration = start.elapsed();
        println!("async-file sequential read [file_size: {} bytes, block_size: {} bytes] costs time: {:?}", file_size, block_size, duration);

        file.flush().await.unwrap();
    });
}

mod runtime {
    use std::sync::Once;

    use async_file::*;
    use async_rt::{wait::WaiterQueue, waiter_loop};
    use io_uring_callback::{Builder, IoUring};
    use lazy_static::lazy_static;

    pub struct Runtime;

    pub const IO_URING_SIZE: usize = 10240;
    pub const PAGE_CACHE_SIZE: usize = 1024 * 25; // 4 MB * 25 = 100 MB
    pub const DIRTY_LOW_MARK: usize = PAGE_CACHE_SIZE / 10 * 3;
    pub const DIRTY_HIGH_MARK: usize = PAGE_CACHE_SIZE / 10 * 7;
    pub const MAX_DIRTY_PAGES_PER_FLUSH: usize = IO_URING_SIZE / 10;

    lazy_static! {
        static ref PAGE_CACHE: PageCache = PageCache::with_capacity(PAGE_CACHE_SIZE);
        static ref FLUSHER: Flusher<Runtime> = Flusher::new();
        static ref WAITER_QUEUE: WaiterQueue = WaiterQueue::new();
        pub static ref RING: IoUring = Builder::new().build(IO_URING_SIZE as u32).unwrap();
    }

    impl AsyncFileRt for Runtime {
        fn io_uring() -> &'static IoUring {
            &RING
        }
        fn page_cache() -> &'static PageCache {
            &PAGE_CACHE
        }

        fn flusher() -> &'static Flusher<Self> {
            &FLUSHER
        }

        fn auto_flush() {
            static INIT: Once = Once::new();
            INIT.call_once(|| {
                async_rt::task::spawn(async {
                    let page_cache = &PAGE_CACHE;
                    let flusher = &FLUSHER;
                    let waiter_queue = &WAITER_QUEUE;
                    waiter_loop!(waiter_queue, {
                        // Start flushing when the # of dirty pages rises above the high watermark
                        if page_cache.num_dirty_pages() < DIRTY_HIGH_MARK {
                            continue;
                        }

                        // Stop flushing until the # of dirty pages falls below the low watermark
                        while page_cache.num_dirty_pages() > DIRTY_LOW_MARK {
                            flusher.flush(MAX_DIRTY_PAGES_PER_FLUSH).await;
                        }
                    });
                });
            });

            if PAGE_CACHE.num_dirty_pages() >= DIRTY_HIGH_MARK {
                WAITER_QUEUE.wake_all();
            }
        }
    }
}

fn init_async_rt() {
    async_rt::config::set_parallelism(1);

    let ring = &runtime::RING;
    unsafe {
        ring.start_enter_syscall_thread();
    }
    let callback = move || {
        ring.trigger_callbacks();
    };
    async_rt::config::set_sched_callback(callback);
}