use super::process::IDLE;
use super::{Thread, ThreadRef};
/// Get and set the current thread/process.
use crate::prelude::*;

pub fn get() -> ThreadRef {
    let current_ptr = CURRENT_THREAD_PTR.with(|cell| cell.get());
    let current_ref = unsafe { Arc::from_raw(current_ptr) };
    let current_ref_clone = current_ref.clone();
    Arc::into_raw(current_ref);
    current_ref_clone
}

pub(super) fn set(thread_ref: ThreadRef) {
    assert!(thread_ref.tid() > 0);
    replace(thread_ref);
}

pub(super) fn reset() -> ThreadRef {
    replace(IDLE.clone())
}

fn replace(thread_ref: ThreadRef) -> ThreadRef {
    let new_thread_ptr = Arc::into_raw(thread_ref);
    let mut old_thread_ptr = CURRENT_THREAD_PTR.with(|cp| cp.replace(new_thread_ptr));
    unsafe { Arc::from_raw(old_thread_ptr) }
}

thread_local! {
    // By default, the current thread is the idle (tid = 0).
    //
    // TODO: figure out why RefCell<ThreadRef> is not working as expected
    static CURRENT_THREAD_PTR: Cell<*const Thread> = {
        Cell::new(Arc::into_raw(IDLE.clone()))
    };
}
