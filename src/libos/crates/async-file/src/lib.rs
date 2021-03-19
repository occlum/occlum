#![cfg_attr(feature = "sgx", no_std)]
#![feature(get_mut_unchecked)]
#![feature(option_unwrap_none)]
#![feature(drain_filter)]
#![feature(slice_fill)]
#![feature(test)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate lazy_static;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_trts;
#[cfg(feature = "sgx")]
extern crate sgx_untrusted_alloc;
#[cfg(test)]
extern crate test;

mod file;
mod page_cache;
mod util;

pub use crate::file::{AsyncFile, AsyncFileRt, Flusher};
pub use crate::page_cache::{AsFd, Page, PageCache, PageHandle, PageState};

// TODO
// - [ ] Use inode number instead of fd to differentiate AsyncFile

#[cfg(test)]
mod tests {
    use async_io::file::{Async, File};
    use io_uring_callback::{Builder, IoUring};
    use lazy_static::lazy_static;

    pub use self::runtime::Runtime;
    use super::*;

    #[test]
    fn write_read_small_file() {
        async_rt::task::block_on(async {
            let path = "tmp.data.hello_world";
            let file = {
                let path = path.to_string();
                let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC;
                let mode = libc::S_IRUSR | libc::S_IWUSR;
                AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
            };
            // The size of this file is considered _small_ given the size of
            // the page cache.
            let input_buf = "hello world\n".to_string().into_bytes().into_boxed_slice();
            file.write_exact_at(0, &input_buf).await.unwrap();
            file.flush().await.unwrap();
            drop(file);

            let file = {
                let path = path.to_string();
                let flags = libc::O_RDONLY;
                let mode = 0;
                AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
            };
            let mut output_vec = Vec::with_capacity(input_buf.len());
            output_vec.resize(input_buf.len(), 0);
            let mut output_buf = output_vec.into_boxed_slice();
            file.read_exact_at(0, &mut output_buf[..]).await.unwrap();

            assert!(output_buf.len() == input_buf.len());
            assert!(output_buf == input_buf);
        });
    }

    #[test]
    fn write_read_large_file() {
        async_rt::task::block_on(async {
            let path = "tmp.data.test_seq_write_read";
            let file = {
                let path = path.to_string();
                let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC;
                let mode = libc::S_IRUSR | libc::S_IWUSR;
                AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
            };

            // The size of this file is considered _large_ given the size of
            // the page cache.
            let data_len = 16 * 1024 * 1024;
            let mut data: Vec<u8> = Vec::with_capacity(data_len);
            for i in 0..data_len {
                let ch = (i % 128) as u8;
                data.push(ch);
            }

            let input_buf = data.into_boxed_slice();
            file.write_exact_at(0, &input_buf).await.unwrap();
            file.flush().await.unwrap();
            drop(file);

            let file = {
                let path = path.to_string();
                let flags = libc::O_RDONLY;
                let mode = 0;
                AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
            };
            let mut output_vec = Vec::with_capacity(input_buf.len());
            output_vec.resize(input_buf.len(), 0);
            let mut output_buf = output_vec.into_boxed_slice();
            file.read_exact_at(0, &mut output_buf[..]).await.unwrap();

            assert!(output_buf.len() == input_buf.len());
            assert!(output_buf == input_buf);
        });
    }

    // #[test]
    // fn bench_random() {
    //     use std::time::{Duration, Instant};
    //     use async_rt::task::JoinHandle;
    //     use rand::Rng;
    //     use std::sync::Arc;

    //     static file_num: usize = 5;
    //     static block_size: usize = 4096 * 2;

    //     let rng = Arc::new(rand::thread_rng());

    //     let mut randoms = Vec::with_capacity(FILE_LEN / block_size * 4);
    //     while randoms.len() < FILE_LEN / block_size * 4 {
    //         randoms.push(rng.gen_range(0..FILE_LEN / block_size));
    //     }
    //     drop(rng);

    //     async_rt::task::block_on(async {
    //         prepare_file(file_num).await;

    //         let mut join_handles: Vec<JoinHandle<i32>> = (0..file_num)
    //             .map(|i| {
    //                 async_rt::task::spawn(async move {
    //                     let start = Instant::now();

    //                     let file = {
    //                         let path = format!("tmp.data.{}", i).to_string();
    //                         let flags = libc::O_RDWR;
    //                         let mode = 0;
    //                         AsyncFile::<Runtime>::open(path.clone(), flags, mode).unwrap()
    //                     };

    //                     let mut vec = vec![0; block_size];
    //                     let mut buf = vec.into_boxed_slice();

    //                     let mut bytes = 0;
    //                     while bytes < FILE_LEN {
    //                         let offset = randoms.pop().unwrap() * block_size;
    //                         let retval = file.read_at(offset, &mut buf[..]).await;
    //                         // assert!(retval as usize == buf.len());
    //                         assert!(retval >= 0);
    //                         bytes += retval as usize;
    //                     }

    //                     while bytes < FILE_LEN {
    //                         let offset = randoms.pop().unwrap() * block_size;
    //                         buf[0] = buf[0] % 128 + 1;
    //                         let retval = file.write_at(offset, &buf[..]).await;
    //                         // assert!(retval as usize == buf.len());
    //                         assert!(retval >= 0);
    //                         bytes += retval as usize;
    //                     }

    //                     file.flush().await;

    //                     let duration = start.elapsed();
    //                     println!("Time elapsed in random task {} [file_size: {}, block_size: {}] is: {:?}", i, FILE_LEN, block_size, duration);
    //                     i as i32
    //                 })
    //             })
    //             .collect();

    //         for (i, join_handle) in join_handles.iter_mut().enumerate() {
    //             assert!(join_handle.await == (i as i32));
    //         }
    //     });
    // }

    mod runtime {
        use async_rt::{wait::WaiterQueue, waiter_loop};

        use super::*;
        use std::sync::Once;

        pub struct Runtime;

        pub const PAGE_CACHE_SIZE: usize = 10; // 10 * 4KB
        pub const DIRTY_LOW_MARK: usize = 3;
        pub const DIRTY_HIGH_MARK: usize = 6;
        pub const MAX_DIRTY_PAGES_PER_FLUSH: usize = 10;

        lazy_static! {
            static ref PAGE_CACHE: PageCache = PageCache::with_capacity(PAGE_CACHE_SIZE);
            static ref FLUSHER: Flusher<Runtime> = Flusher::new();
            static ref WAITER_QUEUE: WaiterQueue = WaiterQueue::new();
            pub static ref RING: IoUring = Builder::new().build(1024).unwrap();
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

    #[ctor::ctor]
    fn auto_init_async_rt() {
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
}
