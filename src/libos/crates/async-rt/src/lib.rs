//! async-rt
//!
//! async-rt is a Rust async / await runtime for std or SGX environment. Support:
//! - Multi-threading: Users can specify the number of threads to use in the runtime
//! to speed up task execution through multiple threads.
//! - Save computing power: When the thread is idle, it will sleep, avoiding busy
//! waiting and wasting computing power.
//! - Priority scheduling: Tasks support different priorities and can be scheduled
//! according to priority. Schedule higher priority tasks first.
//! - Load balancing: Adaptively maintains workloads between threads to ensure
//! load balancing as much as possible.
//! - Timeout: Wait for a task to complete within a specified time, and return if
//! reach timeout.
#![cfg_attr(feature = "sgx", no_std)]
#![feature(thread_local)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_trait_bound)]
#![feature(duration_constants)]
#![feature(get_mut_unchecked)]
#![feature(negative_impls)]
#![feature(dropck_eyepatch)]
#![feature(core_intrinsics)]
#![feature(drain_filter)]
#![feature(arbitrary_enum_discriminant)]
#![feature(test)]
#![allow(dead_code)]
#![feature(maybe_uninit_uninit_array)]
#![feature(new_uninit)]

#[cfg(test)]
extern crate test;

#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
extern crate alloc;
#[macro_use]
extern crate log;

#[cfg(feature = "auto_run")]
extern crate env_logger;
extern crate lazy_static;
#[cfg(not(feature = "sgx"))]
extern crate libc;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
extern crate sgx_untrusted_alloc;

#[cfg(not(feature = "sgx"))]
pub mod bench;
pub mod executor;
mod load_balancer;
mod macros;
pub mod prelude;
pub mod scheduler;
pub mod sync;
pub mod task;
pub mod time;
pub mod util;
pub mod vcpu;
pub mod wait;

// All unit tests
#[cfg(test)]
mod tests {
    use test::Bencher;

    use crate::async_bench_iter;
    use crate::prelude::*;
    use crate::scheduler::SchedEntity;
    use crate::task::JoinHandle;

    lazy_static! {
        static ref TEST_PARALLELISM: u32 =
            std::thread::available_parallelism().unwrap().get() as u32;
    }

    #[test]
    fn test_hello() {
        info!("TEST_PARALLELISM = {:?}", *TEST_PARALLELISM);
        crate::task::block_on(async {
            let tid = crate::task::current::get().tid();
            println!("Hello from task {:?}", tid);
        });
    }

    #[test]
    fn test_yield() {
        crate::task::block_on(async {
            for _ in 0..100 {
                crate::scheduler::yield_now().await;
            }
        });
    }

    #[test]
    fn test_task_locals() {
        use std::cell::Cell;

        task_local! {
            static COUNT: Cell<u32> = Cell::new(0);
        }

        crate::task::block_on(async {
            for _ in 0..100 {
                COUNT.with(|count| {
                    count.set(count.get() + 1);
                })
            }
            assert!(COUNT.with(|count| count.get()) == 100);
        });
    }

    #[test]
    fn test_spawn_and_join() {
        crate::task::block_on(async {
            use crate::task::JoinHandle;
            let mut join_handles: Vec<JoinHandle<i32>> = (0..10)
                .map(|i| {
                    crate::task::spawn(async move {
                        crate::scheduler::yield_now().await;
                        i
                    })
                })
                .collect();

            for (i, join_handle) in join_handles.iter_mut().enumerate() {
                assert!(join_handle.await == (i as i32));
            }
        });
    }

    #[test]
    fn test_affinity() {
        crate::task::block_on(async {
            let current = crate::task::current::get();
            let affinity = current.sched_state().affinity();

            affinity.set(1, true);
            drop(affinity);

            // The new affinity will take effect after the next scheduling
            crate::scheduler::yield_now().await;
            assert!(current.sched_state().affinity().get(1));
        });
    }

    #[test]
    fn test_scheduler() {
        crate::task::block_on(async {
            let task_num = (*TEST_PARALLELISM) * 100;
            let mut join_handles: Vec<JoinHandle<u32>> = (0..task_num)
                .map(|i| {
                    crate::task::spawn(async move {
                        for _ in 0..100 {
                            crate::scheduler::yield_now().await;
                        }
                        i
                    })
                })
                .collect();

            for (i, join_handle) in join_handles.iter_mut().enumerate() {
                assert!(join_handle.await == (i as u32));
            }
        });
    }

    // #[test]
    // // FIXME: enable this test when async Mutex is ready
    // #[ignore]
    // fn test_scheduler_priority() {
    //     crate::task::block_on(async {
    //         let task_num = TEST_PARALLELISM * 100;
    //         let low_handle = spawn_priority_tasks(task_num, SchedPriority::Low);
    //         let normal_handle = spawn_priority_tasks(task_num, SchedPriority::Normal);
    //         let high_handle = spawn_priority_tasks(task_num, SchedPriority::High);

    //         let low_time = low_handle.await;
    //         let normal_time = normal_handle.await;
    //         let high_time = high_handle.await;

    //         // FIXME: check the time when priority task enabled
    //         // assert!(low_time > normal_time);
    //         // assert!(normal_time > high_time);
    //     });
    // }

    // fn spawn_priority_tasks(task_num: u32, priority: SchedPriority) -> JoinHandle<Duration> {
    //     SpawnOptions::new(async move {
    //         let start = std::time::Instant::now();
    //         let join_handles: Vec<JoinHandle<()>> = (0..task_num)
    //             .map(|_| {
    //                 SpawnOptions::new(async move {
    //                     for _ in 0..100u32 {
    //                         crate::scheduler::yield_now().await;
    //                     }
    //                 })
    //                 .priority(priority)
    //                 .spawn()
    //             })
    //             .collect();
    //         for join_handle in join_handles {
    //             join_handle.await;
    //         }
    //         start.elapsed()
    //     })
    //     .priority(SchedPriority::High)
    //     .spawn()
    // }

    #[test]
    // FIXME: enable this test when async Mutex is ready
    #[ignore]
    fn test_scheduler_full() {
        crate::task::block_on(async {
            const MAX_QUEUED_TASKS: usize = 1_000;
            use crate::task::JoinHandle;

            let task_num = (*TEST_PARALLELISM) * MAX_QUEUED_TASKS as u32 * 2;
            let mut join_handles: Vec<JoinHandle<()>> = (0..task_num)
                .map(|_| {
                    crate::task::spawn(async move {
                        for _ in 0..100u32 {
                            crate::scheduler::yield_now().await;
                        }
                    })
                })
                .collect();

            for join_handle in join_handles.iter_mut() {
                join_handle.await;
            }
        });
    }

    #[bench]
    fn bench_spawn_and_join(b: &mut Bencher) {
        async_bench_iter!(b, async move {
            let handle = crate::task::spawn(async {});
            handle.await;
        });
    }

    #[bench]
    fn bench_yield(b: &mut Bencher) {
        async_bench_iter!(b, async move {
            crate::scheduler::yield_now().await;
        });
    }

    #[bench]
    fn bench_task_local(b: &mut Bencher) {
        use std::cell::Cell;
        task_local! {
            static TASK_LOCAL_U32: Cell<u32> = Cell::new(0);
        }

        async_bench_iter!(b, async move {
            black_box(TASK_LOCAL_U32.with(|cell| cell.get()));
        });
    }

    #[ctor::ctor]
    fn auto_init_executor() {
        // crate::config::set_parallelism(TEST_PARALLELISM);
        crate::vcpu::set_total(*TEST_PARALLELISM);
    }
}

/*

lazy_static! {
    static ref FUTEX_TABLE: FutexTable = FutexTable::new();
}

struct FutexTable {
    table: Mutex<HashMap<usize, Vec<Waker>>>,
}

impl FutexTable {
    pub fn new() -> Self {
        Self {
            table: Mutex::new(HashMap::new()),
        }
    }

    pub async fn wait(&self, addr: &AtomicI32, expected_val: i32) -> () {
        {
            let table = self.table.lock().unwrap();
            if addr.load(Ordering::Acquire) != expected_val {
                return;
            }
        }

        FutexWait::new(addr, self).await;
    }

    pub fn wake(&self, addr: &AtomicI32, max_count: usize) -> usize {
        let mut table = self.table.lock().unwrap();
        let addr = addr as *const AtomicI32 as usize;
        let wakers = match table.get_mut(&addr) {
            None => return 0,
            Some(wakers) => wakers,
        };
        let mut count = 0;
        for _ in 0..max_count {
            let waker = match wakers.pop() {
                None => break,
                Some(waker) => waker,
            };
            waker.wake();
            count += 1;
        }
        count
    }
}

struct FutexWait<'a> {
    polled_once: bool,
    addr: &'a AtomicI32,
    futex_table: &'a FutexTable,
}

impl<'a> FutexWait<'a> {
    pub fn new(addr: &'a AtomicI32, futex_table: &'a FutexTable) -> Self {
        let polled_once = false;
        Self {
            polled_once,
            addr,
            futex_table,
        }
    }
}

impl<'a> Future for FutexWait<'a> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let self_ = self.get_mut();
        if self_.polled_once {
            return Poll::Ready(());
        }
        self_.polled_once = true;

        let addr = self_.addr as *const AtomicI32 as usize;
        let mut table = self_.futex_table.table.lock().unwrap();
        let wakers = table
            .entry(addr)
            .or_insert(Vec::new());
        wakers.push(cx.waker().clone());
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! run_tasks {
        ($($async_code: expr),+,) => {{
            let (executor, spawner) = new_executor_and_spawner();

            $(
                spawner.spawn($async_code);
            )*

            drop(spawner);
            executor.run();
        }};
        ($($async_code: expr),+) => {{
            run_tasks!($($async_code),*,)
        }}
    }

    #[test]
    fn test_futex() {
        let futex_val = Arc::new(AtomicI32::new(10));
        let futex_val2 = futex_val.clone();
        run_tasks!(
            wait_wake(futex_val, true),
            wait_wake(futex_val2, false),
        );
    }

    async fn wait_wake(futex_val: Arc<AtomicI32>, wait_on_odd: bool) {
        let futex_val = futex_val.clone();
        loop {
            let val = futex_val.load(Ordering::Acquire);
            if val == 0 {
                break;
            }

            if (val % 2 == 1) ^ wait_on_odd {
                println!("futex wait (val = {:?})", val);
                FUTEX_TABLE.wait(futex_val.deref(), val).await;
            } else {
                let new_val = futex_val.fetch_sub(1, Ordering::Release);
                println!("futex wake (val = {:?})", new_val);
                FUTEX_TABLE.wake(futex_val.deref(), 1);
            }
        }
    }
}
*/
