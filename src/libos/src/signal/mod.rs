//! The signal subsystem.

use crate::prelude::*;

use sig_action::{SigAction, SigActionFlags, SigDefaultAction};

pub use self::c_types::{sigaction_t, siginfo_t, sigset_t, stack_t};
pub use self::constants::*;
pub use self::do_kill::do_kill_from_outside_enclave;
pub use self::do_sigreturn::{deliver_signal, force_signal};
pub use self::sig_dispositions::SigDispositions;
pub use self::sig_num::SigNum;
pub use self::sig_queues::SigQueues;
pub use self::sig_set::SigSet;
pub use self::sig_stack::SigStack;
pub use self::signals::{FaultSignal, KernelSignal, Signal, UserSignal, UserSignalKind};
pub use self::syscalls::*;

mod c_types;
mod do_kill;
mod do_sigaction;
mod do_sigaltstack;
mod do_sigpending;
mod do_sigprocmask;
mod do_sigreturn;
mod do_sigtimedwait;
mod sig_action;
mod sig_dispositions;
mod sig_num;
mod sig_queues;
mod sig_set;
mod sig_stack;
mod signals;
mod syscalls;

pub mod constants;
