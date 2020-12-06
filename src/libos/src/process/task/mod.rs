/// Task is the low-level representation for the execution of a thread.
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::prelude::*;

/// Note: this definition must be in sync with task.h
#[derive(Debug, Default)]
#[repr(C)]
pub struct Task {
    user_rsp: usize,
    user_rip: usize,
    user_fs: AtomicUsize,
}

impl Task {
    pub unsafe fn new(user_rsp: usize, user_rip: usize, user_fs: Option<usize>) -> Result<Task> {
        // Set the default user fsbase to an address on user stack, which is
        // a relatively safe address in case the user program uses %fs before
        // initializing fs base address.
        let user_fs = AtomicUsize::new(user_fs.unwrap_or(user_rsp));

        Ok(Task {
            user_rsp,
            user_rip,
            user_fs,
        })
    }

    pub(super) fn set_user_fs(&self, user_fs: usize) {
        self.user_fs.store(user_fs, Ordering::Relaxed);
    }

    pub fn user_fs(&self) -> usize {
        self.user_fs.load(Ordering::Relaxed)
    }

    pub fn user_rsp(&self) -> usize {
        self.user_rsp
    }

    pub fn user_rip(&self) -> usize {
        self.user_rip
    }
}
