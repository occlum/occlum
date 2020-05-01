//! The signal subsystem.

use crate::prelude::*;

use sig_action::{SigAction, SigActionFlags, SigDefaultAction};

pub use self::c_types::{sigaction_t, sigset_t};
pub use self::constants::*;
pub use self::do_kill::do_kill_from_outside_enclave;
pub use self::do_sigreturn::{deliver_signal, force_signal};
pub use self::sig_dispositions::SigDispositions;
pub use self::sig_num::SigNum;
pub use self::sig_queues::SigQueues;
pub use self::sig_set::SigSet;
pub use self::signals::{FaultSignal, KernelSignal, Signal, UserSignal, UserSignalKind};
pub use self::syscalls::*;

mod c_types;
mod do_kill;
mod do_sigaction;
mod do_sigpending;
mod do_sigprocmask;
mod do_sigreturn;
mod sig_action;
mod sig_dispositions;
mod sig_num;
mod sig_queues;
mod sig_set;
mod signals;
mod syscalls;

pub mod constants;
