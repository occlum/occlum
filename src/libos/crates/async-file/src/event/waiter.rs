#[cfg(feature = "sgx")]
use std::prelude::v1::*;
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, Mutex};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutex as Mutex};

use atomic::{Atomic, Ordering};
use intrusive_collections::intrusive_adapter;
use intrusive_collections::{LinkedList, LinkedListLink};

use crate::event::counter::Counter;
use crate::util::object_id::ObjectId;

/// A waiter.
pub struct Waiter {
    inner: Arc<Waiter_>,
}

struct Waiter_ {
    counter: Counter,
    queue_id: Atomic<ObjectId>,
    link: LinkedListLink,
}

/// A waiter queue.
pub struct WaiterQueue {
    inner: Mutex<WaiterQueue_>,
    queue_id: ObjectId,
}

struct WaiterQueue_ {
    list: LinkedList<LinkedListAdapter>,
}

intrusive_adapter!(LinkedListAdapter =
    Arc<Waiter_>: Waiter_ {
        link: LinkedListLink
    }
);

impl Waiter {
    pub fn new() -> Self {
        let inner = Arc::new(Waiter_::new());
        Self { inner }
    }

    pub async fn wait(&self) {
        self.inner.counter.read().await;
    }
}

impl Waiter_ {
    pub fn new() -> Self {
        let queue_id = Atomic::new(ObjectId::null());
        let counter = Counter::new(0);
        let link = LinkedListLink::new();
        Self {
            counter,
            queue_id,
            link,
        }
    }
}

unsafe impl Sync for Waiter_ {}
unsafe impl Send for Waiter_ {}

impl WaiterQueue {
    pub fn new() -> Self {
        let inner = Mutex::new(WaiterQueue_::new());
        let queue_id = ObjectId::new();
        Self { inner, queue_id }
    }

    pub fn enqueue(&self, waiter: &Waiter) {
        let old_queue_id = waiter.inner.queue_id.swap(self.queue_id, Ordering::Relaxed);
        assert!(old_queue_id == ObjectId::null());

        let mut inner = self.inner.lock().unwrap();
        inner.list.push_back(waiter.inner.clone());
    }

    pub fn dequeue(&self, waiter: &Waiter) {
        let old_queue_id = waiter
            .inner
            .queue_id
            .swap(ObjectId::null(), Ordering::Relaxed);
        assert!(old_queue_id == self.queue_id);

        let mut inner = self.inner.lock().unwrap();
        let mut cursor = unsafe { inner.list.cursor_mut_from_ptr(Arc::as_ptr(&waiter.inner)) };
        let waiter_inner = cursor.remove().unwrap();
        drop(waiter_inner);
    }

    pub fn wake_all(&self) {
        let inner = self.inner.lock().unwrap();
        inner
            .list
            .iter()
            .for_each(|waiter_inner| waiter_inner.counter.write());
    }
}

impl WaiterQueue_ {
    pub fn new() -> Self {
        let list = LinkedList::new(LinkedListAdapter::new());
        Self { list }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use async_rt::task::JoinHandle;
    use std::sync::Arc;

    #[test]
    fn wait_and_then_wake() {
        async_rt::task::block_on(async {
            let waiter_queue = Arc::new(WaiterQueue::new());

            let num_waiters = 10;
            let num_completed = Arc::new(AtomicUsize::new(0));
            let join_handles: Vec<JoinHandle<()>> = (0..num_waiters)
                .map(|_| {
                    let num_completed = num_completed.clone();
                    let waiter_queue = waiter_queue.clone();
                    async_rt::task::spawn(async move {
                        let waiter = Waiter::new();
                        waiter_queue.enqueue(&waiter);
                        waiter.wait().await;
                        waiter_queue.dequeue(&waiter);
                        num_completed.fetch_add(1, Ordering::Release);
                    })
                })
                .collect();

            while num_completed.load(Ordering::Acquire) < num_waiters {
                waiter_queue.wake_all();
                async_rt::sched::yield_().await;
            }

            for join_handle in join_handles {
                join_handle.await;
            }
        });
    }
}
