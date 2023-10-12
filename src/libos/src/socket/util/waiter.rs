use core::hint;
use core::sync::atomic::AtomicU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Weak;
use std::time::Duration;

use io_uring_callback::Fd;
use sgx_untrusted_alloc::UntrustedBox;

use crate::events::HostEventFd;
// use super::host_event_fd::HostEventFd;
use crate::io_uring::SINGLETON;
use crate::prelude::*;

const SPIN_COUNT: usize = 1000;
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

    // /// Return whether a waiter has been waken up.
    // ///
    // /// Once a waiter is waken up, the `wait` or `wait_mut` method becomes
    // /// non-blocking.
    // pub fn is_woken(&self) -> bool {
    //     self.inner.is_woken()
    // }

    // /// Reset a waiter.
    // ///
    // /// After a `Waiter` being waken up, the `reset` method must be called so
    // /// that the `Waiter` can use the `wait` or `wait_mut` methods to sleep the
    // /// current thread again.
    // pub fn reset(&self) {
    //     self.inner.reset();
    // }

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
        unsafe { self.inner.wait(timeout)? }
        Ok(())
    }

    // /// Put the current thread to sleep until being waken up by a waker.
    // ///
    // /// This method is similar to the `wait` method except that the `timeout`
    // /// argument will be updated to reflect the remaining timeout.
    // pub fn wait_mut(&self, timeout: Option<&mut Duration>) -> Result<()> {
    //     self.inner.wait_mut(timeout)
    // }

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

    // /// Wake up waiters in batch, more efficient than waking up one-by-one.
    // pub fn batch_wake<'a, I: Iterator<Item = &'a Waker>>(iter: I) {
    //     Inner::batch_wake(iter);
    // }
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
    /// 0: unlocked  / can wait / is woken
    /// 1: locked, no other threads waiting / is waiting
    /// 2: locked, and other threads waiting (contended) / is waiting (contended)
    state: AtomicU32,
    host_eventfd: Arc<HostEventFd>,
    // val: UntrustedBox<u64>,
}

impl Inner {
    pub fn new() -> Self {
        let state = AtomicU32::new(0);
        let host_eventfd = current!().host_eventfd().clone();
        // let val = UntrustedBox::new(0_u64);
        Self {
            state,
            host_eventfd,
            // val,
        }
    }

    fn spin(&self) -> u32 {
        let mut spin = SPIN_COUNT;
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state != 0 || spin == 0 {
                return state;
            }

            hint::spin_loop();
            spin -= 1;
        }
    }

    #[inline]
    pub unsafe fn wait(&self, timeout: Option<&Duration>) -> Result<()> {
        if self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            self.wait_contended(timeout)?;
        }
        Ok(())
    }

    fn wait_contended(&self, timeout: Option<&Duration>) -> Result<()> {
        let mut state = self.spin();

        // is woken
        if state == 0 {
            match self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => return Ok(()),
                Err(s) => state = s,
            }
        }

        loop {
            if state != 2 && self.state.swap(2, Ordering::Acquire) == 0 {
                return Ok(());
            }

            self.real_wait(timeout);
            state = self.spin();
        }

        Ok(())


        // if self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
        // if self.state.swap(1, Ordering::Relaxed) == 0 {
        //     let mut state = self.spin();
        //     if state == 0 {
        //         return Ok(());
        //     } else {
        //         assert!(state == 1);
        //         self.state.swap(2, Ordering::Relaxed);
        //         // self.state.compare_exchange(1, 2, Ordering::Acquire, Ordering::Relaxed);
        //         self.real_wait(timeout)
        //     }
        // } else {
        //     panic!()
        // }
    }



    // #[inline]
    // pub unsafe fn try_wait(&self) -> bool {
    //     self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok()
    // }

    // pub unsafe fn wait(&self, timeout: Option<&Duration>) -> Result<()> {
    //     if self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
    //         self.wait_contended(timeout)?;
    //     }
    //     Ok(())
    // }

    // fn wait_contended(&self, timeout: Option<&Duration>) -> Result<()> {
    //     let mut state = self.spin();

    //     if state == 0 {
    //         match self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
    //             Ok(_) => return Ok(()),
    //             Err(s) => state = s,
    //         }
    //     }

    //     loop {
    //         if state != 2 && self.state.swap(2, Ordering::Acquire) == 0 {
    //             return Ok(());
    //         }
    //         self.real_wait(timeout)?;
    //         state = self.spin();
    //     }

    // }

    #[inline]
    fn real_wait(&self, timeout: Option<&Duration>) -> Result<()> {
        self.host_eventfd.poll(timeout)
    }

    // pub fn is_woken(&self) -> bool {
    //     self.is_woken.load(Ordering::Acquire)
    // }

    // pub fn is_woken_relaxed(&self) -> bool {
    //     self.is_woken.load(Ordering::Relaxed)
    // }

    // pub fn reset(&self) {
    //     self.is_woken.store(false, Ordering::Release);
    // }

    // pub fn wait(&self, timeout: Option<&Duration>) -> Result<()> {
    //     let mut retry = SPIN_COUNT;
    //     while (retry != 0) && (!self.is_woken_relaxed()) {
    //         hint::spin_loop();
    //         retry -= 1;
    //     }
        
    //     while !self.is_woken_relaxed() {
    //         self.host_eventfd.poll(timeout)?;
    //     }
    //     Ok(())
    // }

    // pub fn wait_mut(&self, timeout: Option<&mut Duration>) -> Result<()> {
    //     let mut remain = timeout.as_ref().map(|d| **d);

    //     // Need to change timeout from `Option<&mut Duration>` to `&mut Option<Duration>`
    //     // so that the Rust compiler is happy about using the variable in a loop.
    //     let ret = self.do_wait_mut(&mut remain);

    //     if let Some(timeout) = timeout {
    //         *timeout = remain.unwrap();
    //     }
    //     ret
    // }

    // fn do_wait_mut(&self, remain: &mut Option<Duration>) -> Result<()> {
    //     while !self.is_woken() {
    //         self.host_eventfd.poll_mut(remain.as_mut())?;
    //     }
    //     Ok(())
    // }

    pub fn wake(&self) {
        if self.state.swap(0, Ordering::Release) == 2 {
            self.host_eventfd.write_u64(1);
        }
        // if self.state.swap(0, Ordering::Release) == 2 {
        //     self.host_eventfd.write_u64(1);
        // }

        // if self
        //     .is_woken
        //     .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        //     .is_ok()
        // {
        //     self.host_eventfd.write_u64(1);

        //     // let host_fd = Fd(self.host_eventfd.host_fd() as _);

        //     // unsafe { self.val.as_mut_ptr().write(1) };
        //     // let io_uring = &*SINGLETON;
        //     // let buf_ptr = self.val.as_ptr() as *const u8;
        //     // unsafe { io_uring.write(host_fd, buf_ptr, std::mem::size_of::<u64>() as u32, 0, 0) };
        // }
    }

    // pub fn batch_wake<'a, I: Iterator<Item = &'a Waker>>(iter: I) {
    //     let host_eventfds = iter
    //         .filter_map(|waker| waker.inner.upgrade())
    //         .filter(|inner| {
    //             inner
    //                 .is_woken
    //                 .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    //                 .is_ok()
    //         })
    //         .map(|inner| inner.host_eventfd.host_fd())
    //         .collect::<Vec<FileDesc>>();
    //     unsafe {
    //         HostEventFd::write_u64_raw_and_batch(&host_eventfds, 1);
    //     }
    // }

    pub fn host_eventfd(&self) -> &HostEventFd {
        &self.host_eventfd
    }
}
