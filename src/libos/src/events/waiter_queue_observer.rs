use std::any::Any;
use std::marker::PhantomData;
use std::sync::Weak;

use super::{Event, Observer, WaiterQueue};
use crate::prelude::*;

/// A Observer associated with a WaiterQueue.
///
/// Once the observer receives any interesting events, it will dequeue and
/// wake up all `Waiters` in the associated `WaiterQueue`.
pub struct WaiterQueueObserver<E: Event> {
    waiter_queue: WaiterQueue,
    phantom: PhantomData<E>,
}

impl<E: Event> WaiterQueueObserver<E> {
    pub fn new() -> Arc<Self> {
        let waiter_queue = WaiterQueue::new();
        let phantom = PhantomData;
        Arc::new(Self {
            waiter_queue,
            phantom,
        })
    }

    pub fn waiter_queue(&self) -> &WaiterQueue {
        &self.waiter_queue
    }
}

impl<E: Event> Observer<E> for WaiterQueueObserver<E> {
    fn on_event(&self, event: &E, _metadata: &Option<Weak<dyn Any + Send + Sync>>) {
        self.waiter_queue.dequeue_and_wake_all();
    }
}
