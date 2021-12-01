use std::collections::VecDeque;
use std::fmt;

use async_rt::task::Tirqs;

use super::constants::*;
use super::{SigNum, SigSet, Signal};
use crate::prelude::*;
use crate::process::{ProcessRef, ThreadRef};

/// Enqueue a process-target signal.
///
/// Enqueuing a signal triggers a TIRQ.
pub fn enqueue_process_signal(process: &ProcessRef, signal: Box<dyn Signal>) {
    let signum = signal.num();
    let mut sig_queues = process.sig_queues().write().unwrap();
    sig_queues.enqueue(signal);

    // Notify the waiter of sigtimedwait
    process.sig_waiters().wake_all();

    // Interrupt the main thread of the process
    // TODO: is it enough to just interrupt the main thread? Interrupting all
    // threads seem to be detrimental to performance, while interrupting one
    // thread may not be enough.
    if let Some(thread) = process.leader_thread() {
        if let Some(task) = thread.task() {
            task.tirqs().put_req(signum.as_tirq_line());
        } else {
            // TODO: fix this race condition
            warn!("potential a signal loss when interrupting a thread before its first scheduling");
        }
    }
}

/// Enqueue a thread-target signal.
///
/// Enqueuing a signal triggers a TIRQ.
pub fn enqueue_thread_signal(thread: &ThreadRef, signal: Box<dyn Signal>) {
    let signum = signal.num();
    let mut sig_queues = thread.sig_queues().write().unwrap();
    sig_queues.enqueue(signal);

    // Notify the waiter of sigtimedwait
    thread.process().sig_waiters().wake_all();

    // Interrupt the thread
    if let Some(task) = thread.task() {
        task.tirqs().put_req(signum.as_tirq_line());
    } else {
        // TODO: fix this race condition
        warn!("potential a signal loss when interrupting a thread before its first scheduling");
    }
}

/// Dequeue a signal that may be delivered to the (current) thread.
///
/// Signals whose signal numbers are within the given mask will not be considered
/// for dequeuing.
///
/// Signals are dequeued in the following priorities, from high to low.
/// 1. Standard signals;
/// 2. Real-time signals;
/// 3. Process-targeted signals;
/// 4. Thread-targeted signals.
///
/// When dequeuing signals, we check whether there are any remaining signals
/// for a specific signal number. If there are none, then the corresponding
/// TIRQ line will be cleared.
///
/// Check out the `async_rt` crate for how the task interrupt mechanism works.
pub fn dequeue_signal(thread: &ThreadRef, mask: SigSet) -> Option<Box<dyn Signal>> {
    debug_assert!(current!().tid() == thread.tid());

    let process = thread.process().clone();
    let mut process_sqs = process.sig_queues().write().unwrap();
    let mut thread_sqs = thread.sig_queues().write().unwrap();
    let mut sigqueues_list = [&mut *process_sqs, &mut *thread_sqs];
    do_dequeue_signal(&mut sigqueues_list, mask)
}

fn do_dequeue_signal(
    sigqueues_list: &mut [&mut SigQueues],
    mask: SigSet,
) -> Option<Box<dyn Signal>> {
    // Fast path: no signals at all
    if sigqueues_list.iter().all(|sq| sq.is_empty()) {
        Tirqs::clear_all_reqs();
        return None;
    }

    // Enumerate all interesting signal numbers
    let interest = !mask;
    for signum in interest.iter() {
        let mut signal = None;
        let mut has_more_signals = false;

        // Try to pop a signal of this signum
        let mut sq_i = 0;
        while sq_i < sigqueues_list.len() {
            let sq = &mut sigqueues_list[sq_i];
            if let Some(_signal) = sq.dequeue(signum) {
                signal = Some(_signal);
                break;
            }
            sq_i += 1;
        }

        // Check if there are any more signals of this signum
        if signal.is_some() {
            while sq_i < sigqueues_list.len() {
                let sq = &mut sigqueues_list[sq_i];
                if sq.can_dequeue(signum) {
                    has_more_signals = true;
                    break;
                }
                sq_i += 1;
            }
        }

        if !has_more_signals {
            Tirqs::clear_req(signum.as_tirq_line());
        }

        if signal.is_some() {
            return signal;
        }
    }

    None
}

/// The signal queues of a thread or a process.
///
/// Each queue keeps signals for a specific signal number.
pub struct SigQueues {
    count: usize,
    std_queues: Vec<Option<Box<dyn Signal>>>,
    rt_queues: Vec<VecDeque<Box<dyn Signal>>>,
}

impl SigQueues {
    pub fn new() -> Self {
        let count = 0;
        let std_queues = (0..COUNT_STD_SIGS).map(|_| None).collect();
        let rt_queues = (0..COUNT_RT_SIGS).map(|_| Default::default()).collect();
        SigQueues {
            count,
            std_queues,
            rt_queues,
        }
    }

    /// Is all signal queues empty?
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Enqueue a signal to the queue that corresponds to the signal number.
    pub fn enqueue(&mut self, signal: Box<dyn Signal>) {
        let signum = signal.num();
        if signum.is_std() {
            // Standard signals
            //
            // From signal(7):
            //
            // Standard signals do not queue.  If multiple instances of a standard
            // signal are generated while that signal is blocked, then only one
            // instance of the signal is marked as pending (and the signal will be
            // delivered just once when it is unblocked).  In the case where a
            // standard signal is already pending, the siginfo_t structure (see
            // sigaction(2)) associated with that signal is not overwritten on
            // arrival of subsequent instances of the same signal.  Thus, the
            // process will receive the information associated with the first
            // instance of the signal.
            let queue = self.get_std_queue_mut(signum);
            if queue.is_some() {
                // If there is already a signal pending, just ignore all subsequent signals
                return;
            }
            *queue = Some(signal);
            self.count += 1;
        } else {
            // Real-time signals
            let queue = self.get_rt_queue_mut(signum);
            queue.push_back(signal);
            self.count += 1;
        }
    }

    /// Dequeue a signal with the given signal number.
    pub fn dequeue(&mut self, signum: SigNum) -> Option<Box<dyn Signal>> {
        if signum.is_std() {
            let queue = self.get_std_queue_mut(signum);
            let signal = queue.take();
            if signal.is_some() {
                self.count -= 1;
            }
            signal
        } else {
            let queue = self.get_rt_queue_mut(signum);
            let signal = queue.pop_front();
            if signal.is_some() {
                self.count -= 1;
            }
            signal
        }
    }

    /// Can a signal with the given signal number be dequeued?
    pub fn can_dequeue(&mut self, signum: SigNum) -> bool {
        if signum.is_std() {
            let queue = self.get_std_queue_mut(signum);
            let signal = queue.take();
            signal.is_some()
        } else {
            let queue = self.get_rt_queue_mut(signum);
            !queue.is_empty()
        }
    }

    /// Returns the sigal numbers of pending signals.
    pub fn pending(&self) -> SigSet {
        let mut pending_sigs = SigSet::new_empty();
        for signum in MIN_STD_SIG_NUM..=MAX_STD_SIG_NUM {
            let signum = unsafe { SigNum::from_u8_unchecked(signum) };
            let queue = self.get_std_queue(signum);
            if queue.is_some() {
                pending_sigs += signum;
            }
        }
        for signum in MIN_RT_SIG_NUM..=MAX_RT_SIG_NUM {
            let signum = unsafe { SigNum::from_u8_unchecked(signum) };
            let queue = self.get_rt_queue(signum);
            if !queue.is_empty() {
                pending_sigs += signum;
            }
        }
        pending_sigs
    }

    fn get_std_queue(&self, signum: SigNum) -> &Option<Box<dyn Signal>> {
        debug_assert!(signum.is_std());
        let idx = (signum.as_u8() - MIN_STD_SIG_NUM) as usize;
        &self.std_queues[idx]
    }

    fn get_rt_queue(&self, signum: SigNum) -> &VecDeque<Box<dyn Signal>> {
        debug_assert!(signum.is_real_time());
        let idx = (signum.as_u8() - MIN_RT_SIG_NUM) as usize;
        &self.rt_queues[idx]
    }

    fn get_std_queue_mut(&mut self, signum: SigNum) -> &mut Option<Box<dyn Signal>> {
        debug_assert!(signum.is_std());
        let idx = (signum.as_u8() - MIN_STD_SIG_NUM) as usize;
        &mut self.std_queues[idx]
    }

    fn get_rt_queue_mut(&mut self, signum: SigNum) -> &mut VecDeque<Box<dyn Signal>> {
        debug_assert!(signum.is_real_time());
        let idx = (signum.as_u8() - MIN_RT_SIG_NUM) as usize;
        &mut self.rt_queues[idx]
    }
}

impl Default for SigQueues {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SigQueues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let signals = self
            .std_queues
            .iter()
            .flatten()
            .chain(self.rt_queues.iter().flatten());
        write!(f, "SigQueues {{ ");
        write!(f, "queue = ");
        f.debug_list().entries(signals).finish();
        write!(f, " }}")
    }
}
