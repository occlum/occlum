use self::runtime::Runtime;
use async_file::*;
use async_rt::task::JoinHandle;
use std::time::Instant;

const CACHE_SIZE: usize = 1024 * 50; // 1024 * 50: 4 MB * 50 (cache hit), or 10 (cache miss)

static mut SEED: u32 = 0;
fn get_random() -> u32 {
    unsafe {
        SEED = SEED * 1103515245 + 12345;
        let hi = SEED >> 16;
        SEED = SEED * 1103515245 + 12345;
        let lo = SEED >> 16;
        return (hi << 16) + lo;
    }
}

fn read_write_bench(
    file_num: usize,
    file_block_size: usize,
    file_total_size: usize,
    is_read: bool,
    is_seq: bool,
    use_fsync: bool,
    use_direct: bool,
    loops: usize,
) {
    async_rt::task::block_on(async move {
        let file_size = file_total_size / file_num;
        // prepare file.
        for i in 0..file_num {
            let file = {
                let path = format!("tmp.data.{}", i).to_string();
                let flags = libc::O_RDWR | libc::O_CREAT | libc::O_TRUNC;
                let mode = libc::S_IRUSR | libc::S_IWUSR;
                AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
            };

            let vec = vec![0; 4096];
            let buf = vec.into_boxed_slice();

            let mut offset = 0;
            while offset < file_size {
                let retval = file.write_at(offset, &buf[..]).await.unwrap();
                offset += retval;
            }

            file.flush().await.unwrap();
        }

        println!("---------------------------bench---------------------------");
        let mb_size = 1024 * 1024;
        let page_size = 4096;
        let io_type = match (is_read, is_seq) {
            (true, true) => "SEQ_READ",
            (true, false) => "RND_READ",
            (false, true) => "SEQ_WRITE",
            (false, false) => "RND_WRITE",
        };
        println!("async-file {} [cache_size: {} MB, file_size: {} MB, file_num: {}, file_block_size: {}, loops: {}, use_fsync: {}, use_direct: {}]", 
                    io_type, CACHE_SIZE * page_size / mb_size, file_size / mb_size, file_num, file_block_size, loops, use_fsync, use_direct);

        let mut join_handles: Vec<JoinHandle<i32>> = (0..file_num)
            .map(|i| {
                async_rt::task::spawn(async move {
                    let file = {
                        let path = format!("tmp.data.{}", i).to_string();
                        let mut flags = libc::O_RDWR;
                        if use_direct {
                            flags |= libc::O_DIRECT;
                        }
                        let mode = 0;
                        AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
                    };

                    let start = Instant::now();

                    let vec = vec![0; file_block_size];
                    let mut buf = vec.into_boxed_slice();
                    for _ in 0..loops {
                        if is_seq {
                            let mut offset = 0;
                            while offset < file_size {
                                if is_read {
                                    let nbytes = file.read_at(offset, &mut buf[..]).await.unwrap();
                                    assert!(nbytes > 0);
                                    offset += nbytes as usize;
                                } else {
                                    let nbytes = file.write_at(offset, &buf[..]).await.unwrap();
                                    assert!(nbytes > 0);
                                    assert!(nbytes == file_block_size);
                                    offset += nbytes;
                                }
                            }
                        } else {
                            let mut cnt = 0;
                            let block_num = file_size / file_block_size;
                            while cnt < file_size {
                                let offset = (get_random() as usize % block_num) * file_block_size;

                                if is_read {
                                    let nbytes = file.read_at(offset, &mut buf[..]).await.unwrap();
                                    assert!(nbytes > 0);
                                    cnt += nbytes as usize;
                                } else {
                                    let nbytes = file.write_at(offset, &buf[..]).await.unwrap();
                                    assert!(nbytes > 0);
                                    cnt += nbytes;
                                }
                            }
                        }

                        if !is_read && use_fsync {
                            file.flush().await.unwrap();
                        }
                    }

                    let duration = start.elapsed();

                    let throughput = ((file_size * loops) / mb_size) as f64
                        / (duration.as_millis() as f64 / 1000.0);
                    println!(
                        "[Task {}] time: {:?}, throughput: {} Mb/s",
                        i, duration, throughput,
                    );

                    file.flush().await.unwrap();

                    i as i32
                })
            })
            .collect();
        for (i, join_handle) in join_handles.iter_mut().enumerate() {
            assert!(join_handle.await == (i as i32));
        }
    });
}

mod runtime {
    use super::*;
    use std::sync::Once;

    use async_rt::{wait::WaiterQueue, waiter_loop};
    use io_uring_callback::{Builder, IoUring};
    use lazy_static::lazy_static;

    pub struct Runtime;

    pub const IO_URING_SIZE: usize = 10240;
    pub const PAGE_CACHE_SIZE: usize = CACHE_SIZE;
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
    let callback = move || {
        ring.trigger_callbacks();
    };
    async_rt::config::set_sched_callback(callback);
}
