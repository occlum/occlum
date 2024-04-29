mod edge;
mod level;
mod synchronizer;

pub use self::edge::EdgeSync;
pub use self::level::LevelSync;
pub use self::synchronizer::Synchronizer;

use super::HostEventFd;
use crate::prelude::*;
use std::{sync::Weak, time::Duration};

/// A waiter enables a thread to sleep.
pub struct Waiter<Sync = LevelSync>
where
    Sync: Synchronizer,
{
    inner: Arc<Sync>,
}

impl<Sync: Synchronizer> Waiter<Sync> {
    /// Create a waiter for the current thread.
    ///
    /// A `Waiter` is bound to the curent thread that creates it: it cannot be
    /// sent to or used by any other threads as the type implements `!Send` and
    /// `!Sync` traits. Thus, a `Waiter` can only put the current thread to sleep.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Sync::new()),
        }
    }

    /// Reset a waiter.
    ///
    /// After a `Waiter` being waken up, the `reset` method must be called so
    /// that the `Waiter` can use the `wait` or `wait_mut` methods to sleep the
    /// current thread again.
    pub fn reset(&self) {
        self.inner.reset();
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
    pub fn wait(&self, timeout: Option<&Duration>) -> Result<()> {
        self.inner.wait(timeout)
    }

    /// Put the current thread to sleep until being waken up by a waker.
    ///
    /// This method is similar to the `wait` method except that the `timeout`
    /// argument will be updated to reflect the remaining timeout.
    pub fn wait_mut(&self, timeout: Option<&mut Duration>) -> Result<()> {
        self.inner.wait_mut(timeout)
    }

    /// Create a waker that can wake up this waiter.
    ///
    /// `WaiterQueue` maintains a list of `Waker` internally to wake up the
    /// enqueued `Waiter`s. So, for users that uses `WaiterQueue`, this method
    /// does not need to be called manually.
    pub fn waker(&self) -> Waker<Sync> {
        Waker {
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// Expose the internal host eventfd.
    ///
    /// This host eventfd should be used by an external user carefully.
    pub fn host_eventfd(&self) -> &HostEventFd {
        self.inner.host_eventfd()
    }
}

impl<S: Synchronizer> !Send for Waiter<S> {}
impl<S: Synchronizer> !Sync for Waiter<S> {}

/// A waker can wake up the thread that its waiter has put to sleep.
pub struct Waker<S = LevelSync>
where
    S: Synchronizer,
{
    inner: Weak<S>,
}

impl<S: Synchronizer> Waker<S> {
    /// Wake up the waiter that creates this waker.
    pub fn wake(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.wake()
        }
    }

    /// Wake up waiters in batch, more efficient than waking up one-by-one.
    pub fn batch_wake<'a, W: 'a + Synchronizer, I: Iterator<Item = &'a Waker<W>>>(iter: I) {
        let host_eventfds = iter
            .filter_map(|waker| waker.inner.upgrade())
            .filter(|inner| inner.wake_cond())
            .map(|inner| inner.host_eventfd().host_fd())
            .collect::<Vec<FileDesc>>();

        unsafe {
            HostEventFd::write_u64_raw_and_batch(&host_eventfds, 1);
        }
    }
}
