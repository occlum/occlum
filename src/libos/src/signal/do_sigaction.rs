use super::constants::*;
use super::{SigAction, SigNum};
use crate::prelude::*;

pub fn do_rt_sigaction(signum: SigNum, new_sa: Option<SigAction>) -> Result<SigAction> {
    debug!(
        "do_rt_sigaction: signum: {:?}, new_sa: {:?}",
        &signum, &new_sa
    );

    if (signum == SIGKILL || signum == SIGSTOP) && new_sa.is_some() {
        return_errno!(
            EINVAL,
            "The actions for SIGKILL or SIGSTOP cannot be changed"
        );
    }

    let thread = current!();
    let process = thread.process();
    let mut sig_dispositions = process.sig_dispositions().write().unwrap();
    let old_sa = sig_dispositions.get(signum);
    if let Some(new_sa) = new_sa {
        sig_dispositions.set(signum, new_sa);
    }
    Ok(old_sa)
}
