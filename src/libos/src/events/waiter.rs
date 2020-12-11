use std::cmp::PartialEq;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Weak;
use std::task::{Context, Poll};
use std::time::Duration;

use super::host_event_fd::HostEventFd;
use crate::prelude::*;

/// A waiter enables a thread to sleep.
pub struct Waiter {
    inner: Arc<SgxMutex<Inner>>,
}

impl Waiter {
    /// Create a waiter for the current thread.
    ///
    /// A `Waiter` is bound to the curent thread that creates it: it cannot be
    /// sent to or used by any other threads as the type implements `!Send` and
    /// `!Sync` traits. Thus, a `Waiter` can only put the current thread to sleep.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SgxMutex::new(Inner::new())),
        }
    }

    /// Return whether a waiter has been waken up.
    ///
    /// Once a waiter is waken up, the `wait` or `wait_mut` method becomes
    /// non-blocking.
    pub fn is_woken(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.is_woken
    }

    /// Reset a waiter.
    ///
    /// After a `Waiter` being waken up, the `reset` method must be called so
    /// that the `Waiter` can use the `wait` or `wait_mut` methods to sleep the
    /// current thread again.
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.is_woken = false;
    }

    /// Put the current thread to sleep until being waken up by a waker.
    ///
    /// The method has three possible return values:
    /// 1. `Ok(())`: The `Waiter` has been waken up by one of its `Waker`;
    /// 2. `Err(e) if e.errno() == Errno::ETIMEDOUT`: Timeout.
    /// 3. `Err(e) if e.errno() == Errno::EINTR`: Interrupted by a signal.
    ///
    /// If the `timeout` argument is `None`, then the second case won't happen,
    /// i.e., the method will block indefinitely.
    pub fn wait(&self, timeout: Option<&Duration>) -> WaiterFuture {
        WaiterFuture::new(&self.inner)
    }

    /// Put the current thread to sleep until being waken up by a waker.
    ///
    /// This method is similar to the `wait` method except that the `timeout`
    /// argument will be updated to reflect the remaining timeout.
    pub fn wait_mut(&self, timeout: Option<&mut Duration>) -> WaiterFuture {
        WaiterFuture::new(&self.inner)
    }

    /// Create a waker that can wake up this waiter.
    ///
    /// `WaiterQueue` maintains a list of `Waker` internally to wake up the
    /// enqueued `Waiter`s. So, for users that uses `WaiterQueue`, this method
    /// does not need to be called manually.
    pub fn waker(&self) -> Waker {
        Waker {
            weak_inner: Arc::downgrade(&self.inner),
        }
    }
}

/// A waker can wake up the thread that its waiter has put to sleep.
#[derive(Clone)]
pub struct Waker {
    weak_inner: Weak<SgxMutex<Inner>>,
}

impl Waker {
    /// Wake up the waiter that creates this waker.
    pub fn wake(&self) {
        let arc_inner = match self.weak_inner.upgrade() {
            None => return,
            Some(inner) => inner,
        };
        let mut inner = arc_inner.lock().unwrap();

        if inner.is_woken {
            return;
        }
        inner.is_woken = true;

        let waker = match inner.core_waker.take() {
            None => return,
            Some(waker) => waker,
        };

        drop(inner);

        waker.wake();
    }
}

impl PartialEq for Waker {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for Waker {}

struct Inner {
    is_woken: bool,
    core_waker: Option<core::task::Waker>,
}

impl Inner {
    pub fn new() -> Self {
        let is_woken = false;
        let core_waker = None;
        Self {
            is_woken,
            core_waker,
        }
    }
}

pub struct WaiterFuture<'a> {
    inner: &'a Arc<SgxMutex<Inner>>,
}

impl<'a> WaiterFuture<'a> {
    fn new(inner: &'a Arc<SgxMutex<Inner>>) -> Self {
        Self { inner }
    }
}

impl<'a> Future for WaiterFuture<'a> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.inner.lock().unwrap();

        if inner.is_woken {
            return Poll::Ready(());
        }

        inner.core_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}
