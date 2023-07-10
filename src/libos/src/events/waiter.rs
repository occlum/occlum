use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Weak;
use std::time::Duration;

use super::host_event_fd::HostEventFd;
use crate::prelude::*;

/// A waiter enables a thread to sleep.
pub struct Waiter {
    inner: Arc<Inner>,
}

impl Waiter {
    /// Create a waiter for the current thread.
    ///
    /// A `Waiter` is bound to the curent thread that creates it: it cannot be
    /// sent to or used by any other threads as the type implements `!Send` and
    /// `!Sync` traits. Thus, a `Waiter` can only put the current thread to sleep.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new()),
        }
    }

    /// Return whether a waiter has been waken up.
    ///
    /// Once a waiter is waken up, the `wait` or `wait_mut` method becomes
    /// non-blocking.
    pub fn is_woken(&self) -> bool {
        self.inner.is_woken()
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
    pub fn waker(&self) -> Waker {
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

impl !Send for Waiter {}
impl !Sync for Waiter {}

/// A waker can wake up the thread that its waiter has put to sleep.
pub struct Waker {
    inner: Weak<Inner>,
}

impl Waker {
    /// Wake up the waiter that creates this waker.
    pub fn wake(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.wake()
        }
    }

    /// Wake up waiters in batch, more efficient than waking up one-by-one.
    pub fn batch_wake<'a, I: Iterator<Item = &'a Waker>>(iter: I) {
        Inner::batch_wake(iter);
    }
}

/// Instruction rearrangement about control dependency
///
/// Such as the following code:
/// fn function(flag: bool, a: i32, b: i32) {
///     if flag { // 1
///         let i = a * b; // 2
///     }
/// }
///
/// Guidelines for compilation optimization：without changing the single-threaded semantics
/// of the program, the execution order of statements can be rearranged. There is a control
/// dependency between flag and i. When the instruction is reordered, step 2 will write the
/// result value to the hardware cache, and when judged to be true, the result value will be
/// written to the variable i. Therefore, controlling dependency does not prevent compiler
/// optimizations
///
/// Note about memory ordering:
/// Here is_woken needs to be synchronized with host_eventfd. The read operation of
/// is_woken needs to see the change of the host_eventfd field. Just `Acquire` or
/// `Release` needs to be used to make all the change of the host_eventfd visible to us.
///
/// The ordering in CAS operations can be `Relaxed`, `Acquire`, `AcqRel` or `SeqCst`,
/// The key is to consider the specific usage scenario. Here fail does not synchronize other
/// variables in the CAS operation, which can use `Relaxed`, and the host_enent needs
/// to be synchronized in success, so `Acquire` needs to be used so that we can see all the
/// changes in the host_eventfd after that.
///
/// Although it is correct to use AcqRel, here I think it is okay to use Acquire, because
/// you don't need to synchronize host_event before is_woken, only later.
struct Inner {
    is_woken: AtomicBool,
    host_eventfd: Arc<HostEventFd>,
}

impl Inner {
    pub fn new() -> Self {
        let is_woken = AtomicBool::new(false);
        let host_eventfd = current!().host_eventfd().clone();
        Self {
            is_woken,
            host_eventfd,
        }
    }

    pub fn is_woken(&self) -> bool {
        self.is_woken.load(Ordering::Acquire)
    }

    pub fn reset(&self) {
        self.is_woken.store(false, Ordering::Release);
    }

    pub fn wait(&self, timeout: Option<&Duration>) -> Result<()> {
        while !self.is_woken() {
            self.host_eventfd.poll(timeout)?;
        }
        Ok(())
    }

    pub fn wait_mut(&self, timeout: Option<&mut Duration>) -> Result<()> {
        let mut remain = timeout.as_ref().map(|d| **d);

        // Need to change timeout from `Option<&mut Duration>` to `&mut Option<Duration>`
        // so that the Rust compiler is happy about using the variable in a loop.
        let ret = self.do_wait_mut(&mut remain);

        if let Some(timeout) = timeout {
            *timeout = remain.unwrap();
        }
        ret
    }

    fn do_wait_mut(&self, remain: &mut Option<Duration>) -> Result<()> {
        while !self.is_woken() {
            self.host_eventfd.poll_mut(remain.as_mut())?;
        }
        Ok(())
    }

    pub fn wake(&self) {
        if self
            .is_woken
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            self.host_eventfd.write_u64(1);
        }
    }

    pub fn batch_wake<'a, I: Iterator<Item = &'a Waker>>(iter: I) {
        let host_eventfds = iter
            .filter_map(|waker| waker.inner.upgrade())
            .filter(|inner| {
                inner
                    .is_woken
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            })
            .map(|inner| inner.host_eventfd.host_fd())
            .collect::<Vec<FileDesc>>();
        unsafe {
            HostEventFd::write_u64_raw_and_batch(&host_eventfds, 1);
        }
    }

    pub fn host_eventfd(&self) -> &HostEventFd {
        &self.host_eventfd
    }
}
