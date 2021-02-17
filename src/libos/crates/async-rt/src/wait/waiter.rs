use atomic::{Atomic, Ordering};
use intrusive_collections::{LinkedList, LinkedListLink};
use object_id::ObjectId;

use crate::prelude::*;

/// A waiter.
pub struct Waiter {
    inner: Arc<WaiterInner>,
}

/// The states of a waiter.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WaiterState {
    Idle,
    Waiting,
    Woken,
}

impl Waiter {
    pub fn new() -> Self {
        let inner = Arc::new(WaiterInner::new());
        Self { inner }
    }

    pub fn state(&self) -> WaiterState {
        self.inner.state()
    }

    pub fn reset(&self) {
        self.inner.state.store(WaiterState::Idle, Ordering::Relaxed);
    }

    pub fn wait(&self) -> WaitFuture<'_> {
        self.inner.wait()
    }

    pub(super) fn inner(&self) -> &Arc<WaiterInner> {
        &self.inner
    }
}

// Accesible by WaiterQueue.
pub(super) struct WaiterInner {
    state: Atomic<WaiterState>,
    waker: Mutex<Option<Waker>>,
    queue_id: Atomic<ObjectId>,
    pub(super) link: LinkedListLink,
}

impl WaiterInner {
    pub fn new() -> Self {
        Self {
            state: Atomic::new(WaiterState::Idle),
            link: LinkedListLink::new(),
            waker: Mutex::new(None),
            queue_id: Atomic::new(ObjectId::null()),
        }
    }

    pub fn state(&self) -> WaiterState {
        self.state.load(Ordering::Relaxed)
    }

    pub fn set_state(&self, new_state: WaiterState) {
        self.state.store(new_state, Ordering::Relaxed)
    }

    pub fn queue_id(&self) -> &Atomic<ObjectId> {
        &self.queue_id
    }

    pub fn wait(&self) -> WaitFuture<'_> {
        WaitFuture::new(self)
    }

    pub fn wake(&self) -> Option<()> {
        let mut waker = self.waker.lock();
        match self.state() {
            WaiterState::Idle => {
                self.set_state(WaiterState::Woken);
                Some(())
            }
            WaiterState::Waiting => {
                self.set_state(WaiterState::Woken);

                let waker = waker.take().unwrap();
                waker.wake();
                Some(())
            }
            WaiterState::Woken => None,
        }
    }
}

unsafe impl Sync for WaiterInner {}
unsafe impl Send for WaiterInner {}

pub struct WaitFuture<'a> {
    waiter: &'a WaiterInner,
}

impl<'a> WaitFuture<'a> {
    fn new(waiter: &'a WaiterInner) -> Self {
        Self { waiter }
    }
}

impl<'a> Future for WaitFuture<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut waker = self.waiter.waker.lock();
        match self.waiter.state() {
            WaiterState::Idle => {
                self.waiter.set_state(WaiterState::Waiting);

                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
            WaiterState::Waiting => {
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
            WaiterState::Woken => {
                debug_assert!(waker.is_none());
                Poll::Ready(())
            }
        }
    }
}
