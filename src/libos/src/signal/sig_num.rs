use std::fmt;

use super::constants::*;
use crate::events::Event;
use crate::prelude::*;

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct SigNum {
    num: u8,
}

impl SigNum {
    pub fn from_u8(num: u8) -> Result<SigNum> {
        if num < MIN_STD_SIG_NUM || num > MAX_RT_SIG_NUM {
            return_errno!(EINVAL, "not an invalid number for signal");
        }
        Ok(unsafe { Self::from_u8_unchecked(num) })
    }

    pub const unsafe fn from_u8_unchecked(num: u8) -> SigNum {
        SigNum { num }
    }

    pub fn as_u8(&self) -> u8 {
        self.num
    }

    pub fn is_std(&self) -> bool {
        self.num <= MAX_STD_SIG_NUM
    }

    pub fn is_real_time(&self) -> bool {
        self.num >= MIN_RT_SIG_NUM
    }
}

macro_rules! std_signum_to_name {
    ( $std_signum: expr, { $( $sig_name: ident = $sig_num_u8: expr ),+, } ) => {
        match $std_signum {
        $(
            $sig_name => stringify!($sig_name),
        )*
            _ => unreachable!(),
        }
    }
}

impl fmt::Debug for SigNum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #![deny(unreachable_patterns)]
        if self.is_std() {
            let name = std_signum_to_name!(*self, {
                SIGHUP    = 1, // Hangup detected on controlling terminal or death of controlling process
                SIGINT    = 2, // Interrupt from keyboard
                SIGQUIT   = 3, // Quit from keyboard
                SIGILL    = 4, // Illegal Instruction
                SIGTRAP   = 5, // Trace/breakpoint trap
                SIGABRT   = 6, // Abort signal from abort(3)
                SIGBUS    = 7, // Bus error (bad memory access)
                SIGFPE    = 8, // Floating-point exception
                SIGKILL   = 9, // Kill signal
                SIGUSR1   = 10, // User-defined signal 1
                SIGSEGV   = 11, // Invalid memory reference
                SIGUSR2   = 12, // User-defined signal 2
                SIGPIPE   = 13, // Broken pipe: write to pipe with no readers; see pipe(7)
                SIGALRM   = 14, // Timer signal from alarm(2)
                SIGTERM   = 15, // Termination signal
                SIGSTKFLT = 16, // Stack fault on coprocessor (unused)
                SIGCHLD   = 17, // Child stopped or terminated
                SIGCONT   = 18, // Continue if stopped
                SIGSTOP   = 19, // Stop process
                SIGTSTP   = 20, // Stop typed at terminal
                SIGTTIN   = 21, // Terminal input for background process
                SIGTTOU   = 22, // Terminal output for background process
                SIGURG    = 23, // Urgent condition on socket (4.2BSD)
                SIGXCPU   = 24, // CPU time limit exceeded (4.2BSD); see setrlimit(2)
                SIGXFSZ   = 25, // File size limit exceeded (4.2BSD); see setrlimit(2)
                SIGVTALRM = 26, // Virtual alarm clock (4.2BSD)
                SIGPROF   = 27, // Profiling timer expired
                SIGWINCH  = 28, // Window resize signal (4.3BSD, Sun)
                SIGIO     = 29, // I/O now possible (4.2BSD)
                SIGPWR    = 30, // Power failure (System V)
                SIGSYS    = 31, // Bad system call (SVr4); see also seccomp(2)
            });
            write!(f, "SigNum (#{} = {})", self.num, name)
        } else {
            write!(f, "SigNum (#{}, real-time)", self.num)
        }
    }
}

impl Event for SigNum {}
