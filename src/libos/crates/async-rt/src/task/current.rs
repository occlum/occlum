use core::cell::Cell;
use core::ptr::{self};

use crate::prelude::*;
use crate::task::Task;

pub fn current() -> Arc<Task> {
    let ptr = CURRENT.get();
    assert!(ptr != ptr::null());
    let current_task = unsafe { Arc::from_raw(ptr) };
    Arc::into_raw(current_task.clone());
    current_task
}

pub(crate) fn set_current(task: Arc<Task>) {
    let last_ptr = CURRENT.replace(Arc::into_raw(task));
    free_task_ptr(last_ptr);
}

pub(crate) fn reset_current() {
    let last_ptr = CURRENT.replace(ptr::null());
    free_task_ptr(last_ptr);
}

fn free_task_ptr(ptr: *const Task) {
    if ptr != ptr::null() {
        let task = unsafe { Arc::from_raw(ptr) };
        drop(task);
    }
}

#[thread_local]
static CURRENT: Cell<*const Task> = Cell::new(ptr::null_mut());
