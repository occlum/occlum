/// File POSIX advisory range locks
use super::*;
use crate::events::{Waiter, WaiterQueue};
use crate::util::sync::rw_lock::RwLockWriteGuard;
use process::pid_t;
use rcore_fs::vfs::AnyExt;

pub use self::builder::RangeLockBuilder;
pub use self::range::{FileRange, OverlapWith, OFFSET_MAX};
use self::range::{FileRangeChange, RangeLockWhence};

mod builder;
mod range;

/// C struct for a file range lock in Libc
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct c_flock {
    /// Type of lock: F_RDLCK, F_WRLCK, or F_UNLCK
    pub l_type: u16,
    /// Where `l_start' is relative to
    pub l_whence: u16,
    /// Offset where the lock begins
    pub l_start: off_t,
    /// Size of the locked area, 0 means until EOF
    pub l_len: off_t,
    /// Process holding the lock
    pub l_pid: pid_t,
}

impl c_flock {
    pub fn copy_from_range_lock(&mut self, lock: &RangeLock) {
        self.l_type = lock.type_ as u16;
        if RangeLockType::F_UNLCK != lock.type_ {
            self.l_whence = RangeLockWhence::SEEK_SET as u16;
            self.l_start = lock.start() as off_t;
            self.l_len = if lock.end() == OFFSET_MAX {
                0
            } else {
                lock.range.len() as off_t
            };
            self.l_pid = lock.owner;
        }
    }
}

/// Kernel representation of file range lock
pub struct RangeLock {
    /// Owner of lock, process holding the lock
    owner: pid_t,
    /// Type of lock, F_RDLCK, F_WRLCK, or F_UNLCK
    type_: RangeLockType,
    /// Range of lock
    range: FileRange,
    /// Optional waiters that are blocking by the lock
    waiters: Option<WaiterQueue>,
}

impl RangeLock {
    pub fn type_(&self) -> RangeLockType {
        self.type_
    }

    pub fn set_type(&mut self, type_: RangeLockType) {
        self.type_ = type_;
    }

    pub fn owner(&self) -> pid_t {
        self.owner
    }

    pub fn conflict_with(&self, other: &Self) -> bool {
        // locks owned by the same process do not conflict
        if self.owner == other.owner {
            return false;
        }
        // locks do not conflict if not overlap
        if self.overlap_with(other).is_none() {
            return false;
        }
        // write lock is exclusive
        if self.type_ == RangeLockType::F_WRLCK || other.type_ == RangeLockType::F_WRLCK {
            return true;
        }
        false
    }

    pub fn overlap_with(&self, other: &Self) -> Option<OverlapWith> {
        self.range.overlap_with(&other.range)
    }

    pub fn merge_with(&mut self, other: &Self) {
        self.range.merge(&other.range).expect("merge range failed");
    }

    pub fn start(&self) -> usize {
        self.range.start()
    }

    pub fn end(&self) -> usize {
        self.range.end()
    }

    pub fn set_start(&mut self, new_start: usize) {
        let change = self.range.set_start(new_start).expect("invalid new start");
        if let FileRangeChange::Shrinked = change {
            self.dequeue_and_wake_all_waiters();
        }
    }

    pub fn set_end(&mut self, new_end: usize) {
        let change = self.range.set_end(new_end).expect("invalid new end");
        if let FileRangeChange::Shrinked = change {
            self.dequeue_and_wake_all_waiters();
        }
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

impl Drop for RangeLock {
    fn drop(&mut self) {
        self.dequeue_and_wake_all_waiters();
    }
}

impl Debug for RangeLock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RangeLock")
            .field("owner", &self.owner)
            .field("type_", &self.type_)
            .field("range", &self.range)
            .finish()
    }
}

impl Clone for RangeLock {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            type_: self.type_.clone(),
            range: self.range.clone(),
            waiters: None,
        }
    }
}

/// List of File POSIX advisory range locks.
///
/// Rule of ordering:
/// Locks are sorted by owner process, then by the starting offset.
///
/// Rule of mergeing:
/// Adjacent and overlapping locks with same owner and type will be merged.
///
/// Rule of updating:
/// New locks with different type will replace or split the overlapping locks
/// if they have same owner.
///
pub struct RangeLockList {
    inner: RwLock<VecDeque<RangeLock>>,
}

impl RangeLockList {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(VecDeque::new()),
        }
    }

    pub fn test_lock(&self, lock: &mut RangeLock) {
        debug!("test_lock with RangeLock: {:?}", lock);
        let list = self.inner.read().unwrap();
        for existing_lock in list.iter() {
            if lock.conflict_with(existing_lock) {
                // Return the information about the conflict lock
                lock.owner = existing_lock.owner;
                lock.type_ = existing_lock.type_;
                lock.range = existing_lock.range;
                return;
            }
        }
        // The lock could be placed at this time
        lock.type_ = RangeLockType::F_UNLCK;
    }

    pub fn set_lock(&self, lock: &RangeLock, is_nonblocking: bool) -> Result<()> {
        debug!(
            "set_lock with RangeLock: {:?}, is_nonblocking: {}",
            lock, is_nonblocking
        );
        loop {
            let mut list = self.inner.write().unwrap();
            if let Some(mut conflict_lock) = list.iter_mut().find(|l| l.conflict_with(lock)) {
                if is_nonblocking {
                    return_errno!(EAGAIN, "lock conflict, try again later");
                }
                // Start to wait
                let waiter = Waiter::new();
                // TODO: Add deadlock detection, and returns EDEADLK
                warn!("Do not support deadlock detection, maybe wait infinitely");
                conflict_lock.enqueue_waiter(&waiter);
                // Ensure that we drop any locks before wait
                drop(list);
                waiter.wait(None)?;
                // Wake up, let's try to set lock again
                continue;
            }
            // No conflict here, let's insert the lock
            return Ok(Self::insert_lock_into_list(&mut list, lock));
        }
    }

    fn insert_lock_into_list(list: &mut RwLockWriteGuard<VecDeque<RangeLock>>, lock: &RangeLock) {
        let first_same_owner_idx = match list.iter().position(|lk| lk.owner() == lock.owner()) {
            Some(idx) => idx,
            None => {
                // Can't find existing locks with same owner.
                list.push_front(lock.clone());
                return;
            }
        };
        // Insert the lock at the start position with same owner, may breaking
        // the rules of RangeLockList.
        // We will handle the inserted lock with next one to adjust the list to
        // obey the rules.
        list.insert(first_same_owner_idx, lock.clone());
        let mut pre_idx = first_same_owner_idx;
        let mut next_idx = pre_idx + 1;
        loop {
            if next_idx >= list.len() {
                break;
            }
            let pre_lock = list[pre_idx].clone();
            let next_lock = list[next_idx].clone();

            if next_lock.owner() != pre_lock.owner() {
                break;
            }
            if next_lock.type_() == pre_lock.type_() {
                // Same type
                if pre_lock.end() < next_lock.start() {
                    break;
                } else if next_lock.end() < pre_lock.start() {
                    list.swap(pre_idx, next_idx);
                    pre_idx += 1;
                    next_idx += 1;
                } else {
                    // Merge adjacent or overlapping locks
                    list[next_idx].merge_with(&pre_lock);
                    list.remove(pre_idx);
                }
            } else {
                // Different type
                if pre_lock.end() <= next_lock.start() {
                    break;
                } else if next_lock.end() <= pre_lock.start() {
                    list.swap(pre_idx, next_idx);
                    pre_idx += 1;
                    next_idx += 1;
                } else {
                    // Split overlapping locks
                    let overlap_with = pre_lock.overlap_with(&next_lock).unwrap();
                    match overlap_with {
                        OverlapWith::ToLeft => {
                            list[next_idx].set_start(pre_lock.end());
                            break;
                        }
                        OverlapWith::InMiddle => {
                            let right_lk = {
                                let mut r_lk = next_lock.clone();
                                r_lk.set_start(pre_lock.end());
                                r_lk
                            };
                            list[next_idx].set_end(pre_lock.start());
                            list.swap(pre_idx, next_idx);
                            list.insert(next_idx + 1, right_lk);
                            break;
                        }
                        OverlapWith::ToRight => {
                            list[next_idx].set_end(pre_lock.start());
                            list.swap(pre_idx, next_idx);
                            pre_idx += 1;
                            next_idx += 1;
                        }
                        OverlapWith::Includes => {
                            // New lock can replace the old one
                            list.remove(next_idx);
                        }
                    }
                }
            }
        }
    }

    pub fn unlock(&self, lock: &RangeLock) {
        debug!("unlock with RangeLock: {:?}", lock);
        let mut list = self.inner.write().unwrap();
        let mut skipped = 0;
        loop {
            let idx = match list
                .iter()
                .skip(skipped)
                .position(|lk| lk.owner() == lock.owner())
            {
                // (idx + skipped) is the original position in list
                Some(idx) => idx + skipped,
                None => break,
            };

            let existing_lock = &mut list[idx];
            let overlap_with = match lock.overlap_with(existing_lock) {
                Some(overlap_with) => overlap_with,
                None => {
                    skipped = idx + 1;
                    continue;
                }
            };

            match overlap_with {
                OverlapWith::ToLeft => {
                    existing_lock.set_start(lock.end());
                    break;
                }
                OverlapWith::InMiddle => {
                    // Split the lock
                    let right_lk = {
                        let mut r_lk = existing_lock.clone();
                        r_lk.set_start(lock.end());
                        r_lk
                    };
                    existing_lock.set_end(lock.start());
                    list.insert(idx + 1, right_lk);
                    break;
                }
                OverlapWith::ToRight => {
                    existing_lock.set_end(lock.start());
                    skipped = idx + 1;
                }
                OverlapWith::Includes => {
                    // The lock can be deleted from the list
                    list.remove(idx);
                    skipped = idx;
                }
            }
        }
    }
}

impl Default for RangeLockList {
    fn default() -> Self {
        Self::new()
    }
}

impl AnyExt for RangeLockList {}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u16)]
pub enum RangeLockType {
    F_RDLCK = 0,
    F_WRLCK = 1,
    F_UNLCK = 2,
}

impl RangeLockType {
    pub fn from_u16(_type: u16) -> Result<Self> {
        Ok(match _type {
            0 => RangeLockType::F_RDLCK,
            1 => RangeLockType::F_WRLCK,
            2 => RangeLockType::F_UNLCK,
            _ => return_errno!(EINVAL, "invalid flock type"),
        })
    }
}
