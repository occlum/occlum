/// Robust futex provide a mechanism that is used in addition to normal futex, for kernel assist of cleanup of held locks on thread exit.
///
/// Actual locking and unlocking is handled entirely by user level code with the existing futex mechanism to wait or wakeup locks.
/// The kernels only essential involvement in robust futex is to remember where the list head is, and to walk the list on thread exit,
/// handling locks still held by the departing thread.
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
        return_errno!(EINVAL, "unknown size of RobustListHead");
    }
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

/// This struct is same with Linux's robust_list
#[repr(C)]
struct RobustList {
    next: *const RobustList,
}

/// This struct is same with Linux's robust_list_head
#[repr(C)]
pub struct RobustListHead {
    /// Linked list of lock entries
    list: RobustList,
    /// Specifies the offset from the address of lock entry to the address of futex
    futex_offset: isize,
    /// Contains transient copy of the address of the lock entry, during list insertion and removal
    list_op_pending: *const RobustList,
}

impl RobustListHead {
    /// Return an iterator for all futexes in the robust list.
    ///
    /// The futex refered to by `list_op_pending`, if any, will be returned as the last item.
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

const ROBUST_LIST_LIMIT: usize = 2048;

pub struct FutexIter<'a> {
    robust_list: &'a RobustListHead,
    entry_ptr: *const RobustList,
    count: usize,
}

impl<'a> FutexIter<'a> {
    fn new(robust_list: &'a RobustListHead) -> Self {
        Self {
            robust_list,
            entry_ptr: robust_list.list.next,
            count: 0,
        }
    }
}

impl<'a> Iterator for FutexIter<'a> {
    type Item = *const i32;

    /// Returns the futex address.
    fn next(&mut self) -> Option<*const i32> {
        if self.count > ROBUST_LIST_LIMIT {
            return None;
        }
        // If it points to the head of the list, then there are no more entries
        while self.entry_ptr != &self.robust_list.list {
            // Avoid excessively long or circular list
            if self.count == ROBUST_LIST_LIMIT {
                break;
            }
            // Invlid pointer
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
        let pending_futex_addr = self.robust_list.pending_futex_addr();
        // If the pending lock exists, mark it as the last item
        if pending_futex_addr.is_some() {
            self.count = ROBUST_LIST_LIMIT + 1;
            return pending_futex_addr;
        }
        None
    }
}

const FUTEX_WAITERS: u32 = 0x8000_0000;
const FUTEX_OWNER_DIED: u32 = 0x4000_0000;
const FUTEX_TID_MASK: u32 = 0x3FFF_FFFF;

/// Wakeup one robust futex owned by the thread
pub fn wake_one_robust_futex(futex_addr: *const i32, tid: pid_t) -> Result<()> {
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
