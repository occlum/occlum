use std::cell::UnsafeCell;

use super::process::IDLE;
use super::{Thread, ThreadRef};
/// Get and set the current thread/process.
use crate::prelude::*;
use crate::*;

/// Ge the thread associated with the current task.
pub fn get() -> ThreadRef {
    let current = CURRENT.with(|current| unsafe { &*current.get() });
    current.as_ref().unwrap().clone()
}

/// Set the thread associated with the current task.
///
/// This method should be only called once at the very beginning of a task
/// that represents an OS thread.
pub(super) unsafe fn set(new_current: ThreadRef) {
    assert!(new_current.tid() > 0);
    let current = CURRENT.with(|current| unsafe { &mut *current.get() });
    *current = Some(new_current);
}

async_rt::task_local! {
    static CURRENT: UnsafeCell<Option<ThreadRef>> = UnsafeCell::new(None);
}
