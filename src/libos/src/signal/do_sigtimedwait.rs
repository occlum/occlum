use std::sync::Weak;
use std::time::Duration;

use async_rt::waiter_loop;

use super::{siginfo_t, SigNum, SigSet, Signal};
use crate::prelude::*;
use crate::process::{ProcessRef, TermStatus, ThreadRef};

pub async fn do_sigtimedwait(interest: SigSet, timeout: Option<&Duration>) -> Result<siginfo_t> {
    debug!(
        "do_rt_sigtimedwait: interest: {:?}, timeout: {:?}",
        interest, timeout,
    );
    // TODO: support timeout
    if let Some(timeout) = timeout {
        warn!("do not support timeout yet");
    }

    let thread = current!();
    let process = thread.process().clone();

    // Interesting, blocked signals
    let interest = {
        let blocked = thread.sig_mask().read().unwrap();
        *blocked & interest
    };

    // Loop until we find a pending signal or reach timeout
    waiter_loop!(process.sig_waiters(), {
        if let Some(signal) = dequeue_pending_signal(&interest, &thread, &process) {
            let siginfo = signal.to_info();
            return Ok(siginfo);
        }
    });
}

fn dequeue_pending_signal(
    interest: &SigSet,
    thread: &ThreadRef,
    process: &ProcessRef,
) -> Option<Box<dyn Signal>> {
    dequeue_process_pending_signal(process, interest)
        .or_else(|| dequeue_thread_pending_signal(thread, interest))
}

fn dequeue_process_pending_signal(
    process: &ProcessRef,
    interest: &SigSet,
) -> Option<Box<dyn Signal>> {
    let blocked = !*interest;
    process.sig_queues().write().unwrap().dequeue(&blocked)
}

fn dequeue_thread_pending_signal(thread: &ThreadRef, interest: &SigSet) -> Option<Box<dyn Signal>> {
    let blocked = !*interest;
    thread.sig_queues().write().unwrap().dequeue(&blocked)
}
