use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::{Waiter, Waker};
use crate::prelude::*;

/// A queue for waiters.
///
/// By using this queue, we can wake up threads in their waiters' enqueue order.
///
/// While the queue is conceptually for `Waiter`s, it internally maintains a list
/// of `Waker`s.
pub struct WaiterQueue {
    count: AtomicUsize,
    wakers: SgxMutex<VecDeque<Waker>>,
}

impl WaiterQueue {
    /// Creates an empty queue for `Waiter`s.
    pub fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            wakers: SgxMutex::new(VecDeque::new()),
        }
    }

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0
    }

    /// Enqueue a waiter.
    ///
    /// It is allowed to enqueue a waiter more than once before it is dequeued.
    /// But this is usually not a good idea. It is the callers' responsibility
    /// to use the API properly.
    pub fn enqueue(&self, waiter: &Waiter) {
        let mut wakers = self.wakers.lock().unwrap();
        wakers.push_back(waiter.waker());
        self.count.fetch_add(1, Ordering::Release);
    }

    /// Dequeue a waiter.
    pub fn dequeue(&self, waiter: &Waiter) {
        let target_waker = waiter.waker();
        let mut wakers = self.wakers.lock().unwrap();
        let mut dequeued_count = 0;
        wakers.retain(|waker| {
            let retain = waker != &target_waker;
            if !retain {
                dequeued_count += 1;
            }
            retain
        });
        self.count.fetch_sub(dequeued_count, Ordering::Release);
    }

    /// Dequeue all waiters and wake up their threads.
    pub fn wake_all(&self) -> usize {
        // The quick path for a common case
        if self.is_empty() {
            return 0;
        }

        // Collect wakers
        let to_wake: Vec<Waker> = {
            let mut wakers = self.wakers.lock().unwrap();
            wakers.iter().map(|waker| waker.clone()).collect()
        };

        // Wake in batch
        Waker::batch_wake(to_wake.iter());
        to_wake.len()
    }
}
