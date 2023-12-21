use super::do_sigprocmask::do_rt_sigprocmask;
use super::do_sigtimedwait::PendingSigWaiter;
use super::{sigset_t, MaskOp, SigNum, SigSet, Signal};
use crate::prelude::*;

pub fn do_sigsuspend(mask: &sigset_t) -> Result<()> {
    debug!("do_sigsuspend: mask: {:?}", mask);

    let thread = current!();
    let process = thread.process().clone();
    let mut original_sig_set = sigset_t::default();

    // Set signal mask
    let op_and_set = Some((MaskOp::SetMask, mask));
    do_rt_sigprocmask(op_and_set, Some(&mut original_sig_set))?;

    let interest = SigSet::from_c(!*mask);
    let pending_sig_waiter = PendingSigWaiter::new(thread, process, interest);

    match pending_sig_waiter.suspend() {
        Ok(_) => {
            // Restore the original signal mask
            let op_and_set = {
                let op = MaskOp::SetMask;
                let set = &original_sig_set;
                Some((op, set))
            };
            do_rt_sigprocmask(op_and_set, None).unwrap();
            Err(errno!(EINTR, "Wait for EINTR signal successfully"))
        }
        Err(_) => {
            // Impossible path
            Err(errno!(EFAULT, "No interesting, pending signal"))
        }
    }
}
