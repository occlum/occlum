use atomic::Ordering;
use std::{sync::atomic::AtomicU32, time::Duration};

use super::{HostEventFd, Synchronizer};
use crate::prelude::*;

const WAIT: u32 = u32::MAX;
const INIT: u32 = 0;
const NOTIFIED: u32 = 1;

pub struct EdgeSync {
    state: AtomicU32,
    host_eventfd: Arc<HostEventFd>,
}

impl Synchronizer for EdgeSync {
    fn new() -> Self {
        Self {
            state: AtomicU32::new(INIT),
            host_eventfd: current!().host_eventfd().clone(),
        }
    }

    fn wait(&self, timeout: Option<&Duration>) -> Result<()> {
        if self.state.fetch_sub(1, Ordering::Acquire) == NOTIFIED {
            return Ok(());
        }
        loop {
            if let Err(error) = self.host_eventfd.poll(timeout) {
                self.state.store(INIT, Ordering::Relaxed);
                return Err(error);
            }

            if self
                .state
                .compare_exchange(NOTIFIED, INIT, Ordering::Acquire, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            } else {
                // Spurious wake up. We loop to try again.
            }
        }
    }

    fn wait_mut(&self, timeout: Option<&mut Duration>) -> Result<()> {
        if self.state.fetch_sub(1, Ordering::Acquire) == NOTIFIED {
            return Ok(());
        }
        let mut remain = timeout.as_ref().map(|d| **d);
        // Need to change timeout from `Option<&mut Duration>` to `&mut Option<Duration>`
        // so that the Rust compiler is happy about using the variable in a loop.
        let ret = self.host_eventfd.poll_mut(remain.as_mut());
        // Wait for something to happen, assuming it's still set to NOTIFIED.
        // This is not just a store, because we need to establish a
        // release-acquire ordering with unpark().
        if self.state.swap(INIT, Ordering::Acquire) == NOTIFIED {
            // Woke up because of unpark().
        } else {
            // Timeout or spurious wake up.
            // We return either way, because we can't easily tell if it was the
            // timeout or not.
        }
        if let Some(timeout) = timeout {
            *timeout = remain.unwrap();
        }
        ret
    }

    fn reset(&self) {
        // do nothing for edge trigger
        ()
    }

    fn wake(&self) {
        if self.wake_cond() {
            self.host_eventfd.write_u64(1);
        }
    }

    fn host_eventfd(&self) -> &HostEventFd {
        &self.host_eventfd
    }

    fn wake_cond(&self) -> bool {
        self.state.swap(NOTIFIED, Ordering::Release) == WAIT
    }
}
