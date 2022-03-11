use super::file_range::{FileRangeChange, OverlapWith};
use super::*;

use std::fmt::Debug;

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

    pub fn set_owner(&mut self, owner: pid_t) {
        self.owner = owner;
    }

    pub fn reset_with(&mut self, other: &Self) {
        self.owner = other.owner;
        self.type_ = other.type_;
        self.range = other.range;
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

    pub fn len(&self) -> usize {
        self.range.len()
    }

    pub fn set_start(&mut self, new_start: usize) {
        let change = self.range.set_start(new_start).expect("invalid new start");
        if let FileRangeChange::Shrinked = change {
            self.wake_all_waiters();
        }
    }

    pub fn set_end(&mut self, new_end: usize) {
        let change = self.range.set_end(new_end).expect("invalid new end");
        if let FileRangeChange::Shrinked = change {
            self.wake_all_waiters();
        }
    }

    pub fn enqueue_waiter(&mut self, waiter: &mut Waiter) {
        if self.waiters.is_none() {
            self.waiters = Some(WaiterQueue::new());
        }
        self.waiters.as_ref().unwrap().enqueue(waiter)
    }

    pub fn wake_all_waiters(&mut self) -> usize {
        if self.waiters.is_some() {
            return self.waiters.as_ref().unwrap().wake_all();
        }
        0
    }
}

impl Drop for RangeLock {
    fn drop(&mut self) {
        self.wake_all_waiters();
    }
}

impl Debug for RangeLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

pub struct RangeLockBuilder {
    // Mandatory field
    type_: Option<RangeLockType>,
    range: Option<FileRange>,
    owner: Option<pid_t>,
    // Optional fields
    waiters: Option<WaiterQueue>,
}

impl RangeLockBuilder {
    pub fn new() -> Self {
        Self {
            owner: None,
            type_: None,
            range: None,
            waiters: None,
        }
    }

    pub fn owner(mut self, owner: pid_t) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn type_(mut self, type_: RangeLockType) -> Self {
        self.type_ = Some(type_);
        self
    }

    pub fn range(mut self, range: FileRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn waiters(mut self, waiters: WaiterQueue) -> Self {
        self.waiters = Some(waiters);
        self
    }

    pub fn build(self) -> Result<RangeLock> {
        let owner = self
            .owner
            .ok_or_else(|| errno!(EINVAL, "owner is mandatory"))?;
        let type_ = self
            .type_
            .ok_or_else(|| errno!(EINVAL, "type_ is mandatory"))?;
        let range = self
            .range
            .ok_or_else(|| errno!(EINVAL, "range is mandatory"))?;
        let waiters = self.waiters;
        Ok(RangeLock {
            owner,
            type_,
            range,
            waiters,
        })
    }
}

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

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
#[repr(u16)]
pub enum RangeLockWhence {
    SEEK_SET = 0,
    SEEK_CUR = 1,
    SEEK_END = 2,
}

impl RangeLockWhence {
    pub fn from_u16(whence: u16) -> Result<Self> {
        Ok(match whence {
            0 => RangeLockWhence::SEEK_SET,
            1 => RangeLockWhence::SEEK_CUR,
            2 => RangeLockWhence::SEEK_END,
            _ => return_errno!(EINVAL, "Invalid whence"),
        })
    }
}
