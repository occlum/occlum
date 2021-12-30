use std::cell::UnsafeCell;

use super::process::IDLE;
use super::{Thread, ThreadRef};
/// Get and set the current thread/process.
use crate::prelude::*;
use crate::*;

/// Get the current thread.
pub fn get() -> ThreadRef {
    try_get().unwrap_or_else(|| IDLE.clone())
}

/// Attempt to get the thread associated with the current task.
fn try_get() -> Option<ThreadRef> {
    let current_opt = CURRENT.try_with(|current| unsafe { &*current.get() });
    current_opt.map_or(None, |current| current.clone())
}

/// Set the thread associated with the current task.
///
/// This method should be only called once at the very beginning of a task
/// that represents an OS thread.
pub unsafe fn set(new_current: ThreadRef) {
    assert!(new_current.tid() > 0);
    let current = CURRENT.with(|current| unsafe { &mut *current.get() });
    debug_assert!(current.is_none());
    *current = Some(new_current);
}

async_rt::task_local! {
    static CURRENT: UnsafeCell<Option<ThreadRef>> = UnsafeCell::new(None);
}
