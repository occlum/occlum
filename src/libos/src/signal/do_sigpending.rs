use super::SigSet;
use crate::prelude::*;

pub fn do_sigpending() -> Result<SigSet> {
    debug!("do_sigpending");

    let thread = current!();
    let process = thread.process();
    let blocked = *thread.sig_mask().read().unwrap();

    let pending = (thread.sig_queues().read().unwrap().pending()
        | process.sig_queues().read().unwrap().pending())
        & blocked;
    Ok(pending)
}
