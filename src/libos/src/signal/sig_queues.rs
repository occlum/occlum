use std::collections::VecDeque;
use std::fmt;

use super::constants::*;
use super::{SigNum, SigSet, Signal};
use crate::events::Notifier;
use crate::prelude::*;

pub struct SigQueues {
    count: usize,
    std_queues: Vec<Option<Box<dyn Signal>>>,
    rt_queues: Vec<VecDeque<Box<dyn Signal>>>,
    notifier: Notifier<SigNum, SigSet>,
}

impl SigQueues {
    const ORDERED_STD_SIGS: [SigNum; COUNT_STD_SIGS] = [
        SIGKILL, SIGTERM, SIGSTOP, SIGCONT, SIGSEGV, SIGILL, SIGHUP, SIGINT, SIGQUIT, SIGTRAP,
        SIGABRT, SIGBUS, SIGFPE, SIGUSR1, SIGUSR2, SIGPIPE, SIGALRM, SIGSTKFLT, SIGCHLD, SIGTSTP,
        SIGTTIN, SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM, SIGPROF, SIGWINCH, SIGIO, SIGPWR,
        SIGSYS,
    ];

    pub fn new() -> Self {
        let count = 0;
        let std_queues = (0..COUNT_STD_SIGS).map(|_| None).collect();
        let rt_queues = (0..COUNT_RT_SIGS).map(|_| Default::default()).collect();
        let notifier = Notifier::new();
        SigQueues {
            count,
            std_queues,
            rt_queues,
            notifier,
        }
    }

    pub fn empty(&self) -> bool {
        self.count == 0
    }

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

        self.notifier.broadcast(&signum);
    }

    pub fn dequeue(&mut self, blocked: &SigSet) -> Option<Box<dyn Signal>> {
        // Fast path for the common case of no pending signals
        if self.empty() {
            return None;
        }

        // Deliver standard signals.
        //
        // According to signal(7):
        // If both standard and real-time signals are pending for a process,
        // POSIX leaves it unspecified which is delivered first. Linux, like
        // many other implementations, gives priority to standard signals in
        // this case.

        // POSIX leaves unspecified which to deliver first if there are multiple
        // pending standard signals. So we are free to define our own. The
        // principle is to give more urgent signals higher priority (like SIGKILL).
        for &signum in &Self::ORDERED_STD_SIGS {
            if blocked.contains(signum) {
                continue;
            }

            let queue = self.get_std_queue_mut(signum);
            let signal = queue.take();
            if signal.is_some() {
                self.count -= 1;
                return signal;
            }
        }

        // If no standard signals, then deliver real-time signals.
        //
        // According to signal (7):
        // Real-time signals are delivered in a guaranteed order.  Multiple
        // real-time signals of the same type are delivered in the order
        // they were sent.  If different real-time signals are sent to a
        // process, they are delivered starting with the lowest-numbered
        // signal.  (I.e., low-numbered signals have highest priority.)
        for signum in MIN_RT_SIG_NUM..=MAX_RT_SIG_NUM {
            let signum = unsafe { SigNum::from_u8_unchecked(signum) };
            if blocked.contains(signum) {
                continue;
            }

            let queue = self.get_rt_queue_mut(signum);
            let signal = queue.pop_front();
            if signal.is_some() {
                self.count -= 1;
                return signal;
            }
        }

        // There must be pending but blocked signals
        None
    }

    pub fn notifier(&self) -> &Notifier<SigNum, SigSet> {
        &self.notifier
    }

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
