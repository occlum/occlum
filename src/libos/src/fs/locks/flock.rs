/// Non-POSIX file advisory lock (FLOCK)
use super::*;
use crate::events::{Waiter, WaiterQueue};
use rcore_fs::vfs::AnyExt;
use std::ptr;
use std::sync::Weak;

/// Kernel representation of FLOCK
pub struct Flock {
    /// Owner of FLOCK, an opened file descriptor holding the lock
    owner: Weak<dyn File>,
    /// Type of lock, SH_LOCK or EX_LOCK
    type_: FlockType,
    /// Optional waiters that are blocking by the lock
    waiters: Option<WaiterQueue>,
}

impl Flock {
    pub fn new(owner: &Arc<dyn File>, type_: FlockType) -> Self {
        Self {
            owner: Arc::downgrade(owner),
            type_,
            waiters: None,
        }
    }

    pub fn owner(&self) -> Option<Arc<dyn File>> {
        Weak::upgrade(&self.owner)
    }

    pub fn same_owner_with(&self, other: &Self) -> bool {
        self.owner.ptr_eq(&other.owner)
    }

    pub fn conflict_with(&self, other: &Self) -> bool {
        if self.same_owner_with(other) {
            return false;
        }
        if self.type_ == FlockType::EX_LOCK || other.type_ == FlockType::EX_LOCK {
            return true;
        }
        false
    }

    pub fn enqueue_waiter(&mut self, waiter: &Waiter) {
        if self.waiters.is_none() {
            self.waiters = Some(WaiterQueue::new());
        }
        self.waiters.as_ref().unwrap().reset_and_enqueue(waiter)
    }

    pub fn dequeue_and_wake_all_waiters(&mut self) -> usize {
        if self.waiters.is_some() {
            return self.waiters.as_ref().unwrap().dequeue_and_wake_all();
        }
        0
    }
}

impl Drop for Flock {
    fn drop(&mut self) {
        self.dequeue_and_wake_all_waiters();
    }
}

impl Debug for Flock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flock")
            .field("owner", &self.owner.as_ptr())
            .field("type_", &self.type_)
            .finish()
    }
}

/// List of Non-POSIX file advisory lock (FLOCK)
pub struct FlockList {
    inner: RwLock<VecDeque<Flock>>,
}

impl FlockList {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(VecDeque::new()),
        }
    }

    pub fn set_lock(&self, mut req_lock: Flock, is_nonblocking: bool) -> Result<()> {
        debug!(
            "set_lock with Flock: {:?}, is_nonblocking: {}",
            req_lock, is_nonblocking
        );

        loop {
            let mut list = self.inner.write().unwrap();
            if let Some(mut conflict_lock) = list.iter_mut().find(|l| req_lock.conflict_with(&l)) {
                if is_nonblocking {
                    return_errno!(EAGAIN, "The file is locked");
                }
                // Start to wait
                let waiter = Waiter::new();
                // FLOCK do not support deadlock detection
                conflict_lock.enqueue_waiter(&waiter);
                // Ensure that we drop any locks before wait
                drop(list);
                waiter.wait(None)?;
                // Wake up, let's try to set lock again
                continue;
            }
            match list.iter().position(|l| req_lock.same_owner_with(&l)) {
                Some(idx) => {
                    std::mem::swap(&mut req_lock, &mut list[idx]);
                }
                None => {
                    list.push_front(req_lock);
                }
            }
            break;
        }
        Ok(())
    }

    pub fn unlock(&self, req_owner: &INodeFile) {
        debug!("unlock with owner: {:?}", req_owner as *const INodeFile);

        let mut list = self.inner.write().unwrap();
        list.retain(|lock| {
            if let Some(owner) = lock.owner() {
                !ptr::eq(
                    Arc::as_ptr(&owner) as *const INodeFile,
                    req_owner as *const INodeFile,
                )
            } else {
                false
            }
        });
    }
}

impl Default for FlockList {
    fn default() -> Self {
        Self::new()
    }
}

impl AnyExt for FlockList {}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u16)]
pub enum FlockType {
    /// Shared lock
    SH_LOCK = 0,
    /// Exclusive lock
    EX_LOCK = 1,
}

impl From<FlockOps> for FlockType {
    fn from(ops: FlockOps) -> Self {
        if ops.contains(FlockOps::LOCK_EX) {
            Self::EX_LOCK
        } else if ops.contains(FlockOps::LOCK_SH) {
            Self::SH_LOCK
        } else {
            panic!("invalid flockops");
        }
    }
}

bitflags! {
    pub struct FlockOps: i32 {
        /// Shared lock
        const LOCK_SH = 1;
        /// Exclusive lock
        const LOCK_EX = 2;
        // Or'd with one of the above to prevent blocking
        const LOCK_NB = 4;
        // Remove lock
        const LOCK_UN = 8;
    }
}

impl FlockOps {
    pub fn from_i32(bits: i32) -> Result<Self> {
        let ops = Self::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid operation"))?;
        if ops.contains(Self::LOCK_SH) {
            if ops.contains(Self::LOCK_EX) || ops.contains(Self::LOCK_UN) {
                return_errno!(EINVAL, "invalid operation");
            }
        } else if ops.contains(Self::LOCK_EX) {
            if ops.contains(Self::LOCK_UN) {
                return_errno!(EINVAL, "invalid operation");
            }
        } else if !ops.contains(Self::LOCK_UN) {
            return_errno!(EINVAL, "invalid operation");
        }

        Ok(ops)
    }
}
