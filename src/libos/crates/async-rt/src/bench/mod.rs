//! A simple benchmark utility for async code.
//!
//! Rust's standard benchmark utility (i.e., `Bencher`) cannot be used to
//! benchmark async Rust code as async Rust code cannot be runned within a sync
//! textual context (as provided by `Bencher`). While there are some crates that can
//! help in this regard, it cannot be ported to SGX easily.
//!
//! To workaround the limitation, we provide `async_bench_iter`, a convenient
//! macro that can benchmark the async Rust code.
//!
//! ```ignore
//! use test::{Bencher};
//! use async_rt::async_bench_iter;
//!
//! #[bench]
//! fn bench_yield(b: &mut Bencher) {
//!     async_bench_iter!(b, async || {
//!         crate::sched::yield_().await;
//!     });
//! }
//! ```
//!
//! The main advantage of `async_bench_iter` is that it is easy to use
//! and leverages `Bencher` under the hood to measure elapsed time and collect
//! performance metrics. Thus, `cargo bench` outputs the benchmark results
//! of a code snippet benched by `async_bench_iter` in the same format as if
//! it was benched by `Bencher`.
//!
//! The downside of `async_bench_iter` is that it incurs some overhead
//! (around 200ns on a 3rd Generation Intel Xeon Scalable Processors at 2.7GHz).
//! So the macro is not suited for measuring extremely light-weight operations.

use std::hint::{self};
use std::result::Result;

use atomic::{Atomic, Ordering::*};

/// A macro to benchmark async Rust code.
#[macro_export]
macro_rules! async_bench_iter {
    ($bencher: expr, async move $code:expr) => {{
        use std::sync::Arc;
        use std::thread::{self};
        use test::{black_box, Bencher};

        use $crate::bench::{BenchState, StateSync};
        use $crate::task::{self};

        // Step 0
        let state_sync = Arc::new(StateSync::new(BenchState::BenchEnd));

        // Spawn the benchmark helper thread
        let handle = {
            let state_sync = state_sync.clone();
            thread::spawn(move || {
                task::block_on(async move {
                    // Step 1
                    state_sync.switch_to(BenchState::BenchStart);
                    loop {
                        // Step 4
                        if let Err(state) = state_sync.try_busy_wait(BenchState::FuncStart, 1000) {
                            match state {
                                // Step 9
                                BenchState::BenchEnd => {
                                    break;
                                }
                                _ => {
                                    $crate::sched::yield_().await;
                                    continue;
                                }
                            }
                        }

                        // Step 5
                        black_box($code);

                        // Step 6
                        state_sync.switch_to(BenchState::FuncEnd);
                    }
                });
            })
        };

        // Step 2
        state_sync.busy_wait(BenchState::BenchStart);

        // Continue with the benchmark initiater thread
        let bencher: &mut Bencher = $bencher;
        bencher.iter({
            let state_sync = state_sync.clone();
            move || {
                // Step 3
                state_sync.switch_to(BenchState::FuncStart);

                // Let the async task execute the bench function...

                // Step 7 (afterwards, repeat Step 3 or go to Step 8)
                state_sync.busy_wait(BenchState::FuncEnd);
            }
        });

        // Step 8
        state_sync.switch_to(BenchState::BenchEnd);

        // Step 10
        handle.join().unwrap();
    }};
}

/// The state of an async benchmark enabled by `async_bench_iter`.
/// This type is not for external use.
#[doc(hidden)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum BenchState {
    BenchStart,
    FuncStart,
    FuncEnd,
    BenchEnd,
}

/// State synchronization between the two threads spawned by `async_bench_iter`.
/// This type is not for external use.
#[doc(hidden)]
pub struct StateSync<S> {
    state: Atomic<S>,
}

impl<S: Copy + PartialEq> StateSync<S> {
    pub fn new(init_state: S) -> Self {
        Self {
            state: Atomic::new(init_state),
        }
    }

    pub fn switch_to(&self, new_state: S) {
        self.state.store(new_state, Relaxed);
    }

    pub fn busy_wait(&self, target_state: S) {
        while self.state.load(Relaxed) != target_state {
            hint::spin_loop();
        }
    }

    pub fn try_busy_wait(&self, target_state: S, mut max_retries: usize) -> Result<(), S> {
        let mut now_state;
        loop {
            now_state = self.state.load(Relaxed);
            if now_state == target_state {
                return Ok(());
            }
            if max_retries == 0 {
                return Err(now_state);
            }
            max_retries -= 1;
            hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    use test::Bencher;

    #[bench]
    fn bench_nop(b: &mut Bencher) {
        // Let's measure the overhead that the async_bench_iter macro incurs.
        async_bench_iter!(b, async move {
            // Do nothing
        });
    }
}
