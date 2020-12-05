use core::cell::Cell;

use crate::prelude::*;
use crate::task::Task;

pub fn current() -> Arc<Task> {
    try_current().unwrap()
}

pub fn try_current() -> Option<Arc<Task>> {
    let ptr = CURRENT.get();
    if ptr == core::ptr::null() {
        return None;
    }
    let current_task = unsafe { Arc::from_raw(ptr) };
    Arc::into_raw(current_task.clone());
    Some(current_task)
}

pub(crate) fn set_current(task: Arc<Task>) {
    let last_ptr = CURRENT.replace(Arc::into_raw(task));
    free_task_ptr(last_ptr);
}

pub(crate) fn reset_current() {
    let last_ptr = CURRENT.replace(core::ptr::null());
    free_task_ptr(last_ptr);
}

fn free_task_ptr(ptr: *const Task) {
    if ptr != core::ptr::null() {
        let task = unsafe { Arc::from_raw(ptr) };
        drop(task);
    }
}

#[thread_local]
static CURRENT: Cell<*const Task> = Cell::new(core::ptr::null_mut());
