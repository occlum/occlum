/// File POSIX advisory locks
use super::*;
use crate::events::{Waiter, WaiterQueue};
use crate::util::sync::rw_lock::RwLockWriteGuard;
use process::pid_t;
use rcore_fs::vfs::{INodeLockList, INodeLockListCreater};

pub use self::builder::FlockBuilder;
pub use self::range::FlockRange;
use self::range::{FlockRangeReport, FlockWhence, RANGE_EOF};

mod builder;
mod range;

/// C struct for a file lock
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
    pub fn copy_from_safe(&mut self, lock: &Flock) {
        self.l_type = lock.type_ as u16;
        if FlockType::F_UNLCK != lock.type_ {
            self.l_whence = FlockWhence::SEEK_SET as u16;
            self.l_start = lock.range.start() as off_t;
            self.l_len = if lock.range.end() == RANGE_EOF {
                0
            } else {
                lock.range.len() as off_t
            };
            self.l_pid = lock.pid;
        }
    }
}

/// Type safe representation of flock
pub struct Flock {
    /// Owner of lock, corresponds to the file table
    owner: ObjectId,
    /// Type of lock, F_RDLCK, F_WRLCK, or F_UNLCK
    type_: FlockType,
    /// Range of lock
    range: FlockRange,
    /// Process holding the lock
    pid: pid_t,
    /// Waiters that are blocking by the lock
    waiters: Option<WaiterQueue>,
    /// Whether the request is non-blocking
    is_nonblocking: bool,
}

impl Flock {
    pub fn type_(&self) -> FlockType {
        self.type_
    }

    pub fn set_type(&mut self, type_: FlockType) {
        self.type_ = type_;
    }

    pub fn is_nonblocking(&self) -> bool {
        self.is_nonblocking
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

    pub fn conflict_with(&self, other: &Self) -> bool {
        // locks owned by the same process do not conflict
        if self.same_owner_with(other) {
            return false;
        }
        // locks do not conflict if not overlap
        if !self.overlap_with(other) {
            return false;
        }
        // write lock is exclusive
        if self.type_ == FlockType::F_WRLCK || other.type_ == FlockType::F_WRLCK {
            return true;
        }
        false
    }

    pub fn same_owner_with(&self, other: &Self) -> bool {
        self.owner == other.owner
    }

    pub fn same_type_with(&self, other: &Self) -> bool {
        self.type_ == other.type_
    }

    pub fn overlap_with(&self, other: &Self) -> bool {
        self.range.overlap_with(&other.range)
    }

    pub fn left_overlap_with(&self, other: &Self) -> bool {
        self.range.left_overlap_with(&other.range)
    }

    pub fn middle_overlap_with(&self, other: &Self) -> bool {
        self.range.middle_overlap_with(&other.range)
    }

    pub fn right_overlap_with(&self, other: &Self) -> bool {
        self.range.right_overlap_with(&other.range)
    }

    pub fn in_front_of(&self, other: &Self) -> bool {
        self.range.in_front_of(&other.range)
    }

    pub fn in_front_of_or_adjacent_before(&self, other: &Self) -> bool {
        self.range.in_front_of_or_adjacent_before(&other.range)
    }

    pub fn merge_range(&mut self, other: &Self) {
        self.range.merge(&other.range).expect("merge range failed");
    }

    pub fn set_start(&mut self, new_start: usize) {
        let report = self.range.set_start(new_start).expect("invalid new start");
        if let FlockRangeReport::Shrink = report {
            self.dequeue_and_wake_all_waiters();
        }
    }

    pub fn set_end(&mut self, new_end: usize) {
        let report = self.range.set_end(new_end).expect("invalid new end");
        if let FlockRangeReport::Shrink = report {
            self.dequeue_and_wake_all_waiters();
        }
    }

    pub fn reset_by(&mut self, lock: &Self) {
        self.owner = lock.owner;
        self.type_ = lock.type_;
        self.range = lock.range;
        self.pid = lock.pid;
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
            .field("owner", &self.owner)
            .field("type_", &self.type_)
            .field("range", &self.range)
            .field("pid", &self.pid)
            .field("is_nonblocking", &self.is_nonblocking)
            .finish()
    }
}

impl Clone for Flock {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            type_: self.type_.clone(),
            range: self.range.clone(),
            pid: self.pid.clone(),
            waiters: None,
            is_nonblocking: self.is_nonblocking.clone(),
        }
    }
}

/// Used to allocate the lock list for INode
pub struct FlockListCreater;

impl INodeLockListCreater for FlockListCreater {
    fn new_empty_list(&self) -> Arc<dyn INodeLockList> {
        Arc::new(FlockList::new())
    }
}

/// File POSIX lock list
/// Rule of ordering: Locks are sorted by owner process, then by starting offset.
/// Rule of mergeing: Adjacent & overlapping locks with same owner and type will be merged.
pub struct FlockList {
    inner: RwLock<VecDeque<Flock>>,
}

impl INodeLockList for FlockList {
    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

impl FlockList {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(VecDeque::new()),
        }
    }

    pub fn test_lock(&self, lock: &mut Flock) -> Result<()> {
        debug!("test_lock with Flock: {:?}", lock);
        let list = self.inner.read().unwrap();
        for existing_lock in list.iter() {
            if lock.conflict_with(existing_lock) {
                // Return the details about the conflict lock
                lock.reset_by(existing_lock);
                return Ok(());
            }
        }
        // The lock could be placed at this time
        lock.set_type(FlockType::F_UNLCK);
        Ok(())
    }

    pub fn set_lock(&self, lock: &Flock) -> Result<()> {
        debug!("set_lock with Flock: {:?}", lock);
        loop {
            let mut list = self.inner.write().unwrap();
            if let Some(mut conflict_lock) = list.iter_mut().find(|l| l.conflict_with(lock)) {
                if lock.is_nonblocking() {
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
            // No conflict here, insert the lock
            return Self::insert_lock_into_list(&mut list, lock);
        }
    }

    fn insert_lock_into_list(
        list: &mut RwLockWriteGuard<VecDeque<Flock>>,
        lock: &Flock,
    ) -> Result<()> {
        let first_same_owner_idx = match list.iter().position(|lk| lk.same_owner_with(lock)) {
            Some(idx) => idx,
            None => {
                // Can't find the old lock with same owner, just insert it.
                list.push_front(lock.clone());
                return Ok(());
            }
        };
        // Insert the lock at the position with same owner, this may break the rules of FlockList,
        // we will handle the inserted lock with next one to make the list to satisfy the rules.
        list.insert(first_same_owner_idx, lock.clone());
        let mut pre_idx = first_same_owner_idx;
        let mut next_idx = pre_idx + 1;
        loop {
            if next_idx >= list.len() {
                break;
            }
            let pre_lock = list[pre_idx].clone();
            let next_lock = list[next_idx].clone();

            if !next_lock.same_owner_with(&pre_lock) {
                break;
            }
            if next_lock.same_type_with(&pre_lock) {
                // Same type
                if pre_lock.in_front_of(&next_lock) {
                    break;
                } else if next_lock.in_front_of(&pre_lock) {
                    list.swap(pre_idx, next_idx);
                    pre_idx += 1;
                    next_idx += 1;
                } else {
                    // Merge adjacent or overlapping locks
                    list[next_idx].merge_range(&pre_lock);
                    list.remove(pre_idx);
                }
            } else {
                // Different type
                if pre_lock.in_front_of_or_adjacent_before(&next_lock) {
                    break;
                } else if next_lock.in_front_of_or_adjacent_before(&pre_lock) {
                    list.swap(pre_idx, next_idx);
                    pre_idx += 1;
                    next_idx += 1;
                } else {
                    // Split overlapping locks
                    if pre_lock.left_overlap_with(&next_lock) {
                        list[next_idx].set_start(pre_lock.range.end() + 1);
                        break;
                    } else if pre_lock.middle_overlap_with(&next_lock) {
                        let right_lk = {
                            let mut r_lk = next_lock.clone();
                            r_lk.set_start(pre_lock.range.end() + 1);
                            r_lk
                        };
                        list[next_idx].set_end(pre_lock.range.start() - 1);
                        list.swap(pre_idx, next_idx);
                        list.insert(next_idx + 1, right_lk);
                        break;
                    } else if pre_lock.right_overlap_with(&next_lock) {
                        list[next_idx].set_end(pre_lock.range.start() - 1);
                        list.swap(pre_idx, next_idx);
                        pre_idx += 1;
                        next_idx += 1;
                    } else {
                        // New lock can replace the old lock
                        list.remove(next_idx);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn unlock(&self, lock: &Flock) -> Result<()> {
        debug!("unlock with Flock: {:?}", lock);
        let mut list = self.inner.write().unwrap();
        let mut skipped = 0;
        loop {
            let idx = match list
                .iter()
                .skip(skipped)
                .position(|lk| lk.same_owner_with(lock) && lk.overlap_with(lock))
            {
                Some(idx) => idx,
                None => break,
            };
            let existing_lock = &mut list[idx];
            if lock.left_overlap_with(existing_lock) {
                existing_lock.set_start(lock.range.end() + 1);
                break;
            } else if lock.middle_overlap_with(existing_lock) {
                // Split the lock
                let right_lk = {
                    let mut r_lk = existing_lock.clone();
                    r_lk.set_start(lock.range.end() + 1);
                    r_lk
                };
                existing_lock.set_end(lock.range.start() - 1);
                list.insert(idx + 1, right_lk);
                break;
            } else if lock.right_overlap_with(existing_lock) {
                existing_lock.set_end(lock.range.start() - 1);
                skipped = idx + 1;
            } else {
                // The lock can be deleted from the list
                list.remove(idx);
                skipped = idx;
            }
        }
        Ok(())
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u16)]
pub enum FlockType {
    F_RDLCK = 0,
    F_WRLCK = 1,
    F_UNLCK = 2,
}

impl FlockType {
    pub fn from_u16(_type: u16) -> Result<Self> {
        Ok(match _type {
            0 => FlockType::F_RDLCK,
            1 => FlockType::F_WRLCK,
            2 => FlockType::F_UNLCK,
            _ => return_errno!(EINVAL, "invalid flock type"),
        })
    }
}
