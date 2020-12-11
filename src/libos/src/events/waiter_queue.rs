use std::collections::LinkedList;
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
    wakers: SgxMutex<LinkedList<Waker>>,
}

impl WaiterQueue {
    /// Creates an empty queue for `Waiter`s.
    pub fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            wakers: SgxMutex::new(LinkedList::new()),
        }
    }

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.count.load(Ordering::SeqCst) == 0
    }

    /// Enqueue a waiter.
    ///
    /// It is allowed to enqueue a waiter more than once before it is dequeued.
    /// But this is usually not a good idea. It is the callers' responsibility
    /// to use the API properly.
    pub fn enqueue(&self, waiter: &Waiter) {
        let mut wakers = self.wakers.lock().unwrap();
        self.count.fetch_add(1, Ordering::SeqCst);
        wakers.push_back(waiter.waker());
    }

    /// Dequeue a waiter.
    pub fn dequeue(&self, waiter: &Waiter) {
        let target_waker = waiter.waker();
        let mut wakers = self.wakers.lock().unwrap();
        let dequeued_count = wakers.drain_filter(|waker| waker == &target_waker).count();
        self.count.fetch_sub(dequeued_count, Ordering::SeqCst);
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
        let mut to_wake = Vec::new();
        let mut wakers = self.wakers.lock().unwrap();
        let mut count = 0;
        while count < max_count {
            let waker = match wakers.pop_front() {
                None => break,
                Some(waker) => waker,
            };
            to_wake.push(waker);
            count += 1;
        }

        // Wake in batch
        Waker::batch_wake(to_wake.iter());
        to_wake.len()
    }
}
