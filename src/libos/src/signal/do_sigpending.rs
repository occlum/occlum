use super::SigSet;
use crate::prelude::*;

pub fn do_sigpending() -> Result<SigSet> {
    debug!("do_sigpending");

    let thread = current!();
    let process = thread.process();
    let pending = (thread.sig_queues().lock().unwrap().pending()
        | process.sig_queues().lock().unwrap().pending())
        & *thread.sig_mask().read().unwrap();
    Ok(pending)
}
