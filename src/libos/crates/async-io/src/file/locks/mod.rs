use crate::prelude::*;
use async_rt::wait::{Waiter, WaiterQueue};
use libc::{off_t, pid_t};

pub use self::file_range::{FileRange, OFFSET_MAX};
pub use self::range_lock::{RangeLock, RangeLockBuilder, RangeLockType, RangeLockWhence};
pub use self::range_lock_list::RangeLockList;

mod file_range;
mod range_lock;
mod range_lock_list;

/// C struct for a file range lock in Libc
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct flock_c {
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

impl flock_c {
    pub fn copy_from_range_lock(&mut self, lock: &RangeLock) {
        self.l_type = lock.type_() as u16;
        if RangeLockType::F_UNLCK != lock.type_() {
            self.l_whence = RangeLockWhence::SEEK_SET as u16;
            self.l_start = lock.start() as off_t;
            self.l_len = if lock.end() == OFFSET_MAX {
                0
            } else {
                lock.len() as off_t
            };
            self.l_pid = lock.owner();
        }
    }
}
