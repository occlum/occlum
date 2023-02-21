use std::sync::Weak;
use std::time::Duration;

use async_rt::waiter_loop;

use super::sig_queues::dequeue_signal;
use super::{siginfo_t, SigNum, SigSet, Signal};
use crate::prelude::*;
use crate::process::{ProcessRef, TermStatus, ThreadRef};

pub async fn do_sigtimedwait(interest: SigSet, timeout: Option<&Duration>) -> Result<siginfo_t> {
    debug!(
        "do_rt_sigtimedwait: interest: {:?}, timeout: {:?}",
        interest, timeout,
    );

    let thread = current!();
    let process = thread.process().clone();

    // Interesting, blocked signals
    let interest = {
        let blocked = thread.sig_mask();
        blocked & interest
    };

    // If the timespec structure pointed to by timeout is zero-valued
    // and if none of the signals specified by set are pending,
    // then sigtimedwait() shall return immediately with an error.
    // If timeout is the null pointer, the behavior is unspecified in Man but
    // it is block wait in glibc.
    if timeout.is_some() && timeout.unwrap().is_zero() {
        if let Some(signal) = dequeue_signal(&thread, !interest) {
            let siginfo = signal.to_info();
            return Ok(siginfo);
        } else {
            return Err(errno!(EAGAIN, "no interesting, pending signal"));
        }
    }

    let mut timeout = timeout.cloned();
    // Loop until we find a pending signal or reach timeout or get interrupted
    waiter_loop!(process.sig_waiters(), timeout, {
        if let Some(signal) = dequeue_signal(&thread, !interest) {
            let siginfo = signal.to_info();
            return Ok(siginfo);
        }
    })
    .map_err(|_| errno!(EAGAIN, "no interesting, pending signal"))
}
