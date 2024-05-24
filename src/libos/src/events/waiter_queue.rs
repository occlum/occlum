use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{LevelSync, Synchronizer, Waiter, Waker};
use crate::prelude::*;

/// A queue for waiters.
///
/// By using this queue, we can wake up threads in their waiters' enqueue order.
///
/// While the queue is conceptually for `Waiter`s, it internally maintains a list
/// of `Waker`s.
///
/// Note about memory ordering:
/// Here count needs to be synchronized with wakers. The read operation of count
/// needs to see the change of the waker field. Just `Acquire` or `Release` needs
/// to be used to make all the change of the wakers visible to us.
///
/// Regarding the usage of functions like fetch_add and fetch_sub, they perform
/// atomic addition or subtraction operations. The memory ordering parameter for
/// these functions can be chosen from options such as `Relaxed`, `Acquire`, `Release`,
/// `AcqRel` and `SeqCst`. It is important to select the appropriate memory ordering
/// based on the corresponding usage scenario.
///
/// In this code snippet, the count variable is synchronized with the wakers field.
/// In this case, we only need to ensure that waker.lock() occurs before count.
/// Although it is safer to use AcqRel，here using `Release` would be enough.
pub struct WaiterQueue<Sync: Synchronizer = LevelSync> {
    count: AtomicUsize,
    wakers: Mutex<VecDeque<Waker<Sync>>>,
}

impl<Sync: Synchronizer> WaiterQueue<Sync> {
    /// Creates an empty queue for `Waiter`s.
    pub fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            wakers: Mutex::new(VecDeque::new()),
        }
    }

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        // Here is_empty function is only used in line 76 below. And when calling this, it
        // doesn't need to synchronize with the wakers. Therefore, Relaxed can be enough.
        self.count.load(Ordering::Relaxed) == 0
    }

    /// Reset a waiter and enqueue it.
    ///
    /// It is allowed to enqueue a waiter more than once before it is dequeued.
    /// But this is usually not a good idea. It is the callers' responsibility
    /// to use the API properly.
    pub fn reset_and_enqueue(&self, waiter: &Waiter<Sync>) {
        waiter.reset();

        let mut wakers = self.wakers.lock();
        self.count.fetch_add(1, Ordering::Release);
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
            let mut wakers = self.wakers.lock();
            let max_count = max_count.min(wakers.len());
            let to_wake: Vec<Waker<Sync>> = wakers.drain(..max_count).collect();
            self.count.fetch_sub(to_wake.len(), Ordering::Release);
            to_wake
        };

        // Wake in batch
        Waker::<Sync>::batch_wake(to_wake.iter());
        to_wake.len()
    }
}
