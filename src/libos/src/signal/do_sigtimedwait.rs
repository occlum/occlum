use std::sync::Weak;
use std::time::Duration;

use super::{siginfo_t, SigNum, SigSet, Signal};
use crate::events::{Observer, Waiter, WaiterQueueObserver};
use crate::prelude::*;
use crate::process::{ProcessRef, TermStatus, ThreadRef};
use crate::waiter_loop;

pub fn do_sigtimedwait(interest: SigSet, timeout: Option<&Duration>) -> Result<siginfo_t> {
    debug!(
        "do_rt_sigtimedwait: interest: {:?}, timeout: {:?}",
        interest, timeout,
    );

    let thread = current!();
    let process = thread.process().clone();

    // Interesting, blocked signals
    let interest = {
        let blocked = thread.sig_mask().read().unwrap();
        *blocked & interest
    };

    let signal = match timeout {
        None => dequeue_pending_signal(&interest, &thread, &process)
            .ok_or_else(|| errno!(EAGAIN, "no interesting, pending signal"))?,
        Some(timeout) => {
            let pending_sig_waiter = PendingSigWaiter::new(thread, process, interest);
            pending_sig_waiter.wait(timeout).map_err(|e| {
                if e.errno() == Errno::EINTR {
                    return e;
                }
                errno!(EAGAIN, "no interesting, pending signal")
            })?
        }
    };

    let siginfo = signal.to_info();
    Ok(siginfo)
}

struct PendingSigWaiter {
    thread: ThreadRef,
    process: ProcessRef,
    interest: SigSet,
    observer: Arc<WaiterQueueObserver<SigNum>>,
}

impl PendingSigWaiter {
    pub fn new(thread: ThreadRef, process: ProcessRef, interest: SigSet) -> Arc<Self> {
        let observer = WaiterQueueObserver::new();

        let weak_observer = Arc::downgrade(&observer) as Weak<dyn Observer<_>>;
        thread.sig_queues().read().unwrap().notifier().register(
            weak_observer.clone(),
            Some(interest),
            None,
        );
        process.sig_queues().read().unwrap().notifier().register(
            weak_observer,
            Some(interest),
            None,
        );

        Arc::new(Self {
            thread,
            process,
            interest,
            observer,
        })
    }

    pub fn wait(&self, timeout: &Duration) -> Result<Box<dyn Signal>> {
        // Repeat trying to dequeue a pending signal from the current process or thread
        let err_res = waiter_loop!(
            {
                if let Some(signal) =
                    dequeue_pending_signal(&self.interest, &self.thread, &self.process)
                {
                    return Ok(signal);
                }
            },
            self.observer.waiter_queue(),
            Some(timeout)
        );

        // Do not try again if some error is encountered. There are only
        // two possible errors: ETIMEDOUT or EINTR.
        let err = err_res.as_ref().unwrap_err();
        // When interrupted, it is possible that the interrupting signal happens
        // to be an interesting and pending signal. So we attempt to dequeue again.
        if err.errno() == Errno::EINTR {
            if let Some(signal) =
                dequeue_pending_signal(&self.interest, &self.thread, &self.process)
            {
                return Ok(signal);
            }
        }
        err_res
    }
}

impl Drop for PendingSigWaiter {
    fn drop(&mut self) {
        let weak_observer = Arc::downgrade(&self.observer) as Weak<dyn Observer<_>>;
        self.thread
            .sig_queues()
            .read()
            .unwrap()
            .notifier()
            .unregister(&weak_observer);
        self.process
            .sig_queues()
            .read()
            .unwrap()
            .notifier()
            .unregister(&weak_observer);
    }
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
