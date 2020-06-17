//! The termination status of a process or thread.

use crate::signal::SigNum;
use sgx_tstd::sync::SgxMutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ForcedExitStatus {
    exited: AtomicBool,
    status: SgxMutex<Option<TermStatus>>,
}

impl ForcedExitStatus {
    pub fn new() -> Self {
        Self {
            exited: AtomicBool::new(false),
            status: SgxMutex::new(None),
        }
    }

    pub fn is_forced_to_exit(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    pub fn force_exit(&self, status: TermStatus) {
        let mut old_status = self.status.lock().unwrap();
        // set the bool after getting the status lock
        self.exited.store(true, Ordering::SeqCst);
        old_status.get_or_insert(status);
    }

    pub fn term_status(&self) -> Option<TermStatus> {
        *self.status.lock().unwrap()
    }
}

// TODO: support core dump
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TermStatus {
    Exited(u8),
    Killed(SigNum),
    //Dumped(SigNum),
}

impl TermStatus {
    /// Return as a 32-bit integer encoded as specified in wait(2) man page.
    pub fn as_u32(&self) -> u32 {
        match *self {
            TermStatus::Exited(status) => (status as u32) << 8,
            TermStatus::Killed(signum) => (signum.as_u8() as u32),
            //TermStatus::Dumped(signum) => (signum.as_u8() as u32) | 0x80,
        }
    }
}
