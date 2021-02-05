#![cfg_attr(any(not(any(test, feature = "auto_run")), feature = "sgx"), no_std)]
#![feature(const_fn)]
#![feature(thread_local)]

#[cfg(all(feature = "sgx", feature = "auto_run"))]
#[macro_use]
extern crate sgx_tstd as std;
extern crate alloc;
extern crate bit_vec;
#[macro_use]
extern crate lazy_static;
//#[macro_use]
//extern crate log;
extern crate flume;
extern crate spin;

pub mod executor;
mod macros;
pub mod prelude;
pub mod sched;
pub mod task;

// All unit tests
#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn test_hello() {
        crate::task::block_on(async {
            let tid = crate::task::current::get().tid();
            println!("Hello from task {:?}", tid);
        });
    }

    #[test]
    fn test_yield() {
        crate::task::block_on(async {
            for _ in 0..100 {
                crate::sched::yield_().await;
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
                        crate::sched::yield_().await;
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
            use crate::sched::Affinity;

            let current = crate::task::current::get();

            let mut affinity = current.sched_info().affinity().write();
            assert!(affinity.is_full());

            let new_affinity = {
                let mut new_affinity = Affinity::new_empty();
                new_affinity.set(1, true);
                new_affinity
            };
            *affinity = new_affinity.clone();
            drop(affinity);

            // The new affinity will take effect after the next scheduling
            crate::sched::yield_().await;

            assert!(*current.sched_info().affinity().read() == new_affinity);
        });
    }

    #[ctor::ctor]
    fn auto_init_executor() {
        crate::executor::set_parallelism(3);
    }

    mod logger {
        use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

        #[ctor::ctor]
        fn auto_init() {
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(LevelFilter::Info))
                .expect("failed to init the");
        }

        static LOGGER: SimpleLogger = SimpleLogger;

        struct SimpleLogger;

        impl log::Log for SimpleLogger {
            fn enabled(&self, metadata: &Metadata) -> bool {
                metadata.level() <= Level::Info
            }

            fn log(&self, record: &Record) {
                if self.enabled(record.metadata()) {
                    println!("[{}] {}", record.level(), record.args());
                }
            }

            fn flush(&self) {}
        }
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
