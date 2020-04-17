/// Implementation of signals generated from various sources.
mod fault;
mod kernel;
mod user;

pub use self::fault::FaultSignal;
pub use self::kernel::KernelSignal;
pub use self::user::{UserSignal, UserSignalKind};

use super::c_types::siginfo_t;
use super::SigNum;
use crate::prelude::*;

pub trait Signal: Send + Sync + Debug {
    /// Returns the number of the signal.
    fn num(&self) -> SigNum;

    /// Returns the siginfo_t that gives more details about a signal.
    fn to_info(&self) -> siginfo_t;
}
