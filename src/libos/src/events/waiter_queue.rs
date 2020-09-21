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
        self.count.load(Ordering::SeqCst) == 0
    }

    /// Reset a waiter and enqueue it.
    ///
    /// It is allowed to enqueue a waiter more than once before it is dequeued.
    /// But this is usually not a good idea. It is the callers' responsibility
    /// to use the API properly.
    pub fn reset_and_enqueue(&self, waiter: &Waiter) {
        waiter.reset();

        let mut wakers = self.wakers.lock().unwrap();
        self.count.fetch_add(1, Ordering::SeqCst);
        wakers.push_back(waiter.waker());
    }

    /// Dequeue a waiter and wake up its thread.
    pub fn dequeue_and_wake_one(&self) -> usize {
        self.dequeue_and_wake_nr(1)
    }

    /// Dequeue all waiters and wake up their threads.
    pub fn dequeue_and_wake_all(&self) -> usize {
        self.dequeue_and_wake_nr(usize::MAX)
    }

    /// Deuque a maximum numer of waiters and wake up their threads.
    pub fn dequeue_and_wake_nr(&self, max_count: usize) -> usize {
        // The quick path for a common case
        if self.is_empty() {
            return 0;
        }

        // Dequeue wakers
        let to_wake = {
            let mut wakers = self.wakers.lock().unwrap();
            let max_count = max_count.min(wakers.len());
            let to_wake: Vec<Waker> = wakers.drain(..max_count).collect();
            self.count.fetch_sub(to_wake.len(), Ordering::SeqCst);
            to_wake
        };

        // Wake in batch
        Waker::batch_wake(to_wake.iter());
        to_wake.len()
    }
}
