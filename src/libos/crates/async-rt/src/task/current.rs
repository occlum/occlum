use core::cell::Cell;
use core::ptr::{self};

use crate::prelude::*;
use crate::task::Task;

pub fn get() -> Arc<Task> {
    try_get().unwrap()
}

pub fn try_get() -> Option<Arc<Task>> {
    let ptr = CURRENT.get();
    if ptr == ptr::null() {
        return None;
    }
    let current_task = unsafe { Arc::from_raw(ptr) };

    // The memory would free in function free_task_ptr
    #[allow(unused_must_use)]
    {
        Arc::into_raw(current_task.clone());
    }
    Some(current_task)
}

pub(crate) fn set(task: Arc<Task>) {
    let last_ptr = CURRENT.replace(Arc::into_raw(task));
    free_task_ptr(last_ptr);
}

pub(crate) fn reset() {
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

pub(crate) fn set_vcpu_id(vcpu_id: u32) {
    VCPU_ID.store(vcpu_id, Ordering::Relaxed);
}

pub fn get_vcpu_id() -> u32 {
    VCPU_ID.load(Ordering::Relaxed)
}

#[thread_local]
static VCPU_ID: AtomicU32 = AtomicU32::new(0);
