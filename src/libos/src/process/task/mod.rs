/// Task is the low-level representation for the execution of a thread.
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::prelude::*;

pub use self::exec::{enqueue, enqueue_and_exec, exec};

mod exec;

/// Note: this definition must be in sync with task.h
#[derive(Debug, Default)]
#[repr(C)]
pub struct Task {
    kernel_rsp: usize,
    kernel_stack_base: usize,
    kernel_stack_limit: usize,
    kernel_fs: usize,
    user_rsp: usize,
    user_stack_base: usize,
    user_stack_limit: usize,
    user_fs: AtomicUsize,
    user_entry_addr: usize,
    saved_state: usize, // struct jmpbuf*
}

impl Task {
    pub unsafe fn new(
        user_entry_addr: usize,
        user_rsp: usize,
        user_stack_base: usize,
        user_stack_limit: usize,
        user_fs: Option<usize>,
    ) -> Result<Task> {
        if !(user_stack_base >= user_rsp && user_rsp > user_stack_limit) {
            return_errno!(EINVAL, "Invalid user stack");
        }

        // Set the default user fsbase to an address on user stack, which is
        // a relatively safe address in case the user program uses %fs before
        // initializing fs base address.
        let user_fs = AtomicUsize::new(user_fs.unwrap_or(user_stack_limit));

        Ok(Task {
            user_entry_addr,
            user_rsp,
            user_stack_base,
            user_stack_limit,
            user_fs,
            ..Default::default()
        })
    }

    pub(super) fn set_user_fs(&self, user_fs: usize) {
        self.user_fs.store(user_fs, Ordering::SeqCst);
    }

    pub fn user_fs(&self) -> usize {
        self.user_fs.load(Ordering::SeqCst)
    }
}
