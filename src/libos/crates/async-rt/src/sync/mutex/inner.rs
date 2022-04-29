use super::hint;
/// This implementation makes reference to musl libc's design but only keep the basic
/// functionality. Recursive, error-checking, priority-inheritance or robust-list
/// are not supported yet.
use super::*;

use std::sync::atomic::AtomicU64;

use crate::wait::{Waiter, WaiterQueue};

/// This struct can gurantee there is at most one thread accessing the data.
/// `status` indicates the status of this mutex.
/// `waiters` indicates the number of waiters waiting the mutex.
///
/// There are three states of this mutex:
/// - Free: No one is holding the lock. Init state and can also be set by the thread who release the lock.
/// - Locked: One thread is holding the lock. Set by the thread who acquires the lock.
/// - LockedWithWaiters: One thread is holding the lock and some threads are waiting. Set by the waiting thread.
#[derive(Debug)]
pub(super) struct MutexInner {
    status: AtomicLockStatus,
    waiters: AtomicU64,
    waiter_queue: WaiterQueue,
}

// This struct is the atomic wrapper for LockStatus.
#[derive(Debug)]
struct AtomicLockStatus(AtomicU64);

#[derive(Debug, Copy, Clone)]
#[repr(u64)]
enum LockStatus {
    Free = 0,
    Locked = 1,
    LockedWithWaiters = 2,
}

impl MutexInner {
    pub(super) fn new() -> MutexInner {
        MutexInner {
            status: AtomicLockStatus::new(),
            waiters: AtomicU64::new(0),
            waiter_queue: WaiterQueue::new(),
        }
    }

    pub(super) fn try_lock(&self) -> Result<()> {
        if self.status.try_set_lock().is_ok() {
            Ok(())
        } else {
            Err(errno!(EBUSY, "the lock is held by other threads"))
        }
    }

    pub(super) async fn lock(&self) {
        if let Ok(_) = self.try_lock() {
            return;
        }

        // In musl's implmenetation, this is `100`. Considering more overhead in SGX environment,
        // here we make it bigger.
        const SPIN_COUNT: usize = 1000;

        // Spin for a short while if no one is waiting but the lock is held.
        let mut spins = SPIN_COUNT;
        while spins != 0
            && self.status.is_locked()
            // Can't reorder here. `Relaxed` is enough.
            && self.waiters.load(Ordering::Relaxed) == 0
        {
            hint::spin_loop();
            spins -= 1;
        }

        loop {
            if let Ok(_) = self.try_lock() {
                return;
            }

            // In try_set_lock_with_waiters, `AcqRel` will make sure this increment happens before. Thus, `Relaxed` can be used here.
            self.waiters.fetch_add(1, Ordering::Relaxed);

            // Ignore the result here. If the state transition fails, the next wait will not block.
            let _ = self.status.try_set_lock_with_waiters();

            // Wait for unlock.
            self.wait().await;

            self.waiters.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub(super) fn unlock(&self) {
        // `set_free` will make sure this happens before. Thus, `Relaxed` can be used here.
        let waiters = self.waiters.load(Ordering::Relaxed);
        self.status.set_free();

        if waiters != 0 {
            self.wake_one_waiter();
        }
    }

    async fn wait(&self) {
        let mut waiter = Waiter::new();

        let mut locked_waiter_queue = self.waiter_queue.inner().lock();
        // Check the status value again
        if !self.status.is_locked_with_waiters() {
            return;
        }

        locked_waiter_queue.enqueue(&mut waiter);

        drop(locked_waiter_queue);
        let _ = waiter.wait().await;

        self.waiter_queue.dequeue(&mut waiter);
    }

    fn wake_one_waiter(&self) {
        self.waiter_queue.wake_one();
    }
}

// For AtomicLockStatus, global ordering is not needed. `Acquire` and `Release` are enough for the atomic operations.
impl AtomicLockStatus {
    fn new() -> Self {
        Self(AtomicU64::new(LockStatus::new() as u64))
    }

    fn is_free(&self) -> bool {
        self.0.load(Ordering::Acquire) == LockStatus::Free as u64
    }

    fn is_locked(&self) -> bool {
        self.0.load(Ordering::Acquire) != LockStatus::Free as u64
    }

    fn is_locked_with_waiters(&self) -> bool {
        self.0.load(Ordering::Acquire) == LockStatus::LockedWithWaiters as u64
    }

    fn try_set_lock_with_waiters(&self) -> Result<()> {
        if let Err(_) = self.0.compare_exchange(
            LockStatus::Locked as u64,
            LockStatus::LockedWithWaiters as u64,
            Ordering::AcqRel,
            Ordering::Relaxed, // We don't care failure thus make it `Relaxed`.
        ) {
            return_errno!(EAGAIN, "try set lock with waiters failed");
        }
        Ok(())
    }

    fn try_set_lock(&self) -> Result<()> {
        if let Err(_) = self.0.compare_exchange(
            LockStatus::Free as u64,
            LockStatus::Locked as u64,
            Ordering::AcqRel,
            Ordering::Relaxed, // We don't care failure thus make it `Relaxed`.
        ) {
            return_errno!(EBUSY, "mutex is locked");
        }
        Ok(())
    }

    fn set_free(&self) {
        self.0.store(LockStatus::Free as u64, Ordering::Release);
    }
}

impl LockStatus {
    fn new() -> Self {
        LockStatus::Free
    }

    fn is_locked_with_waiters(&self) -> bool {
        *self as u64 == LockStatus::LockedWithWaiters as u64
    }
}

impl TryFrom<u64> for LockStatus {
    type Error = Error;

    fn try_from(v: u64) -> Result<Self> {
        match v {
            x if x == LockStatus::Free as u64 => Ok(LockStatus::Free),
            x if x == LockStatus::Locked as u64 => Ok(LockStatus::Locked),
            x if x == LockStatus::LockedWithWaiters as u64 => Ok(LockStatus::LockedWithWaiters),
            _ => return_errno!(EINVAL, "Invalid lock status"),
        }
    }
}
