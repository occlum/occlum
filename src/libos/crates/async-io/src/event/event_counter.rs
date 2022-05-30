use std::borrow::BorrowMut;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_rt::{wait::WaiterQueue, waiter_loop};

use crate::prelude::*;

/// A counter for wait and wakeup.
///
/// The APIs of EventCounter are similar to that of Liunx's eventfd.
pub struct EventCounter {
    counter: AtomicU64,
    waiters: WaiterQueue,
}

impl EventCounter {
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
            waiters: WaiterQueue::new(),
        }
    }

    pub async fn read(&self) -> Result<u64> {
        waiter_loop!(&self.waiters, {
            let val = self.counter.swap(0, Ordering::Relaxed);
            if val > 0 {
                return Ok(val);
            }
        })
    }

    pub async fn read_timeout<T: BorrowMut<Duration>>(
        &self,
        mut timeout: Option<&mut T>,
    ) -> Result<u64> {
        waiter_loop!(&self.waiters, timeout, {
            let val = self.counter.swap(0, Ordering::Relaxed);
            if val > 0 {
                return Ok(val);
            }
        })
    }

    pub fn write(&self) {
        self.counter.fetch_add(1, Ordering::Relaxed);
        self.waiters.wake_one();
    }
}

impl std::fmt::Debug for EventCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventCounter")
            .field("counter", &self.counter.load(Ordering::Relaxed))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn write_then_read() {
        async_rt::task::block_on(async {
            let counter = EventCounter::new();
            counter.write();
            assert!(counter.read().await.unwrap() == 1);
        });
    }

    #[test]
    fn read_then_write() {
        async_rt::task::block_on(async {
            let counter = Arc::new(EventCounter::new());

            // Spawn a child task that reads the counter
            let handle = {
                let counter = counter.clone();
                async_rt::task::spawn(async move {
                    assert!(counter.read().await.unwrap() == 1);
                })
            };

            // Make sure that the child executes first
            let _20ms = std::time::Duration::from_millis(20);
            std::thread::sleep(_20ms);
            async_rt::sched::yield_().await;

            // Wake up the child task
            counter.write();
            handle.await;
        });
    }
}
