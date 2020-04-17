//! The termination status of a process or thread.

use crate::signal::SigNum;

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
