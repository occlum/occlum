/// Robust futexes provide a mechanism that is used in addition to normal futex,
/// for kernel assist of cleanup of held locks on thread exit.
///
/// Actual locking and unlocking is handled entirely by user level code with the
/// existing futex mechanism to wait or wakeup locks.
/// The kernels only essential involvement in robust futex is to remember where
/// the list head is, and to walk the list on thread exit, handling locks still
/// held by the departing thread.
/// Ref: https://www.kernel.org/doc/html/latest/locking/robust-futex-ABI.html
///
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::prelude::*;
use crate::util::mem_util::from_user::*;

pub fn do_set_robust_list(list_head_ptr: *mut RobustListHead, len: usize) -> Result<()> {
    debug!(
        "set_robust_list: list_head_ptr: {:?}, len: {}",
        list_head_ptr, len
    );
    if std::mem::size_of::<RobustListHead>() != len {
        return_errno!(EINVAL, "invalid size for RobustListHead");
    }
    // We do not check if the pointer is a valid user space pointer, deferring
    // it in waking the robust list. If the pointer is invalid, we just stop
    // waking the robust list.
    let robust_list = NonNull::new(list_head_ptr);
    let current = current!();
    current.set_robust_list(robust_list);
    Ok(())
}

pub fn do_get_robust_list(tid: pid_t) -> Result<*mut RobustListHead> {
    debug!("get_robust_list: tid: {}", tid);
    let thread = if tid == 0 {
        current!()
    } else {
        super::table::get_thread(tid)?
    };
    let robust_list_ptr = thread
        .robust_list()
        .map(|robust_list| robust_list.as_ptr())
        .unwrap_or(std::ptr::null_mut());
    Ok(robust_list_ptr)
}

/// This struct is same as Linux's robust_list
#[repr(C)]
struct RobustList {
    next: *const RobustList,
}

/// This struct is same as Linux's robust_list_head
#[repr(C)]
pub struct RobustListHead {
    /// Linked list of lock entries
    ///
    /// If it points to the head of the list, then it is the end of the list.
    /// If it is an invalid user space pointer or a null pointer, stop iterating
    /// the list.
    list: RobustList,
    /// Specifies the offset from the address of the lock entry to the address
    /// of the futex.
    futex_offset: isize,
    /// Contains transient copy of the address of the lock entry, during list
    /// insertion and removal.
    list_op_pending: *const RobustList,
}

impl RobustListHead {
    /// Return an iterator for all futexes in the robust list.
    ///
    /// The futex referred to by `list_op_pending`, if any, will be returned as
    /// the last item.
    pub fn futexes<'a>(&'a self) -> FutexIter<'a> {
        FutexIter::new(self)
    }

    /// Return the pending futex address if exist
    fn pending_futex_addr(&self) -> Option<*const i32> {
        if self.list_op_pending.is_null() {
            None
        } else {
            Some(unsafe { self.futex_addr(self.list_op_pending) })
        }
    }

    /// Get the futex address
    unsafe fn futex_addr(&self, entry_ptr: *const RobustList) -> *const i32 {
        (entry_ptr as *const u8).offset(self.futex_offset) as *const i32
    }
}

const ROBUST_LIST_LIMIT: isize = 2048;

pub struct FutexIter<'a> {
    robust_list: &'a RobustListHead,
    entry_ptr: *const RobustList,
    count: isize,
}

impl<'a> FutexIter<'a> {
    fn new(robust_list: &'a RobustListHead) -> Self {
        Self {
            robust_list,
            entry_ptr: robust_list.list.next,
            count: 0,
        }
    }

    // The `self.count` is normally a positive value used to iterate the list
    // to avoid excessively long or circular list, we use a special value -1
    // to represent the end of the Iterator.
    fn set_end(&mut self) {
        self.count = -1;
    }

    fn is_end(&self) -> bool {
        self.count < 0
    }
}

impl<'a> Iterator for FutexIter<'a> {
    type Item = *const i32;

    /// Returns the futex address.
    fn next(&mut self) -> Option<*const i32> {
        if self.is_end() {
            return None;
        }

        // Iterate the linked list
        while self.entry_ptr != &self.robust_list.list {
            // Avoid excessively long or circular list
            if self.count == ROBUST_LIST_LIMIT {
                break;
            }
            // Invalid pointer, stop iterating the robust list
            if check_ptr(self.entry_ptr).is_err() {
                return None;
            }
            // A pending lock might already be on the list
            let futex_addr = if self.entry_ptr != self.robust_list.list_op_pending {
                Some(unsafe { self.robust_list.futex_addr(self.entry_ptr) })
            } else {
                None
            };
            self.entry_ptr = unsafe { (*self.entry_ptr).next };
            self.count += 1;
            if futex_addr.is_some() {
                return futex_addr;
            }
        }

        // End of iterating the linked list
        // If the pending futex exists, return it as the last one
        self.set_end();
        self.robust_list.pending_futex_addr()
    }
}

const FUTEX_WAITERS: u32 = 0x8000_0000;
const FUTEX_OWNER_DIED: u32 = 0x4000_0000;
const FUTEX_TID_MASK: u32 = 0x3FFF_FFFF;

/// Wakeup one robust futex owned by the thread
pub fn wake_robust_futex(futex_addr: *const i32, tid: pid_t) -> Result<()> {
    let futex_val = {
        check_ptr(futex_addr)?;
        unsafe { AtomicU32::from_mut(&mut *(futex_addr as *mut u32)) }
    };
    let mut old_val = futex_val.load(Ordering::SeqCst);
    loop {
        // This futex may held by another thread, do nothing
        if old_val & FUTEX_TID_MASK != tid {
            break;
        }
        let new_val = (old_val & FUTEX_WAITERS) | FUTEX_OWNER_DIED;
        if let Err(cur_val) =
            futex_val.compare_exchange(old_val, new_val, Ordering::SeqCst, Ordering::SeqCst)
        {
            // The futex value has changed, let's retry with current value
            old_val = cur_val;
            continue;
        }
        // Wakeup one waiter
        if futex_val.load(Ordering::SeqCst) & FUTEX_WAITERS != 0 {
            debug!("wake robust futex addr: {:?}", futex_addr);
            super::do_futex::futex_wake(futex_addr, 1)?;
        }
        break;
    }
    Ok(())
}
