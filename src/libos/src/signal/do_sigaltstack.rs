use super::sig_stack::{SigStack, SigStackFlags, MINSIGSTKSZ};
use crate::prelude::*;
use crate::syscall::CpuContext;

pub fn do_sigaltstack(new_ss: &Option<SigStack>, curr_user_ctxt: &CpuContext) -> Result<SigStack> {
    debug!("do_sigaltstack: new_ss:{:?}", new_ss);
    let thread = current!();
    let mut sig_stack = thread.sig_stack().lock().unwrap();
    let old_ss = if let Some(sig_stack) = *sig_stack {
        // Deny to update the stack when we are on the stack
        if new_ss.is_some() && sig_stack.contains(curr_user_ctxt.rsp as usize) {
            return_errno!(EPERM, "thread is on signal stack currently");
        }

        // Retrieve the old signal stack information
        let flags = if sig_stack.contains(curr_user_ctxt.rsp as usize) {
            SigStackFlags::SS_ONSTACK
        } else {
            SigStackFlags::EMPTY
        };
        let mut ss: SigStack = Default::default();
        ss.update(sig_stack.sp(), flags, sig_stack.size());
        ss
    } else {
        Default::default()
    };

    // Set or update the signal stack if necessary
    if let Some(new_ss) = new_ss {
        *sig_stack = if new_ss.flags() == SigStackFlags::SS_DISABLE {
            None
        } else {
            if new_ss.size() < MINSIGSTKSZ {
                return_errno!(ENOMEM, "the new alternate signal stack is too small");
            }
            if (new_ss.flags() == SigStackFlags::SS_AUTODISARM) {
                warn!("The SS_AUTODISARM flag is not supported yet");
            }
            Some(*new_ss)
        };
    }
    Ok(old_ss)
}
