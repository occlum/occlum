use super::constants::*;
use super::do_sigtimedwait::PendingSigWaiter;
use super::{sigset_t, MaskOp, SigNum, SigSet, Signal};
use crate::prelude::*;

pub fn do_sigsuspend(mask: &SigSet) -> Result<()> {
    debug!("do_sigsuspend: mask: {:?}", mask);

    let thread = current!();
    let process = thread.process().clone();

    // Set signal mask
    let update_mask = {
        let mut set = *mask;
        // According to man pages, "it is not possible to block SIGKILL or SIGSTOP.
        // Attempts to do so are silently ignored."
        set -= SIGKILL;
        set -= SIGSTOP;
        set
    };

    let mut curr_mask = thread.sig_mask().write().unwrap();
    let prev_mask = *curr_mask;
    *curr_mask = update_mask;
    drop(curr_mask);

    // Suspend for interest signal
    let interest = !update_mask;
    let pending_sig_waiter = PendingSigWaiter::new(thread.clone(), process, interest);

    let err = match pending_sig_waiter.suspend() {
        Ok(_) => {
            errno!(EINTR, "Wait for EINTR signal successfully")
        }
        Err(_) => {
            // Impossible path
            errno!(EFAULT, "No interesting, pending signal")
        }
    };

    // Restore the original signal mask
    let mut curr_mask = thread.sig_mask().write().unwrap();
    *curr_mask = prev_mask;

    Err(err)
}
