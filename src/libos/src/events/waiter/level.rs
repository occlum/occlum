use atomic::Ordering;
use std::{sync::atomic::AtomicBool, time::Duration};

use super::{HostEventFd, Synchronizer};
use crate::prelude::*;

pub struct LevelSync {
    is_woken: AtomicBool,
    host_eventfd: Arc<HostEventFd>,
}

impl Synchronizer for LevelSync {
    fn new() -> Self {
        Self {
            is_woken: AtomicBool::new(false),
            host_eventfd: current!().host_eventfd().clone(),
        }
    }

    fn reset(&self) {
        self.is_woken.store(false, Ordering::Release);
    }

    fn wait(&self, timeout: Option<&Duration>) -> Result<()> {
        while !self.is_woken() {
            self.host_eventfd.poll(timeout)?;
        }
        Ok(())
    }

    fn wait_mut(&self, timeout: Option<&mut Duration>) -> Result<()> {
        let mut remain = timeout.as_ref().map(|d| **d);
        // Need to change timeout from `Option<&mut Duration>` to `&mut Option<Duration>`
        // so that the Rust compiler is happy about using the variable in a loop.

        while !self.is_woken() {
            self.host_eventfd.poll_mut(remain.as_mut())?;
        }

        if let Some(timeout) = timeout {
            *timeout = remain.unwrap();
        }
        Ok(())
    }

    fn wake(&self) {
        if self.wake_cond() {
            self.host_eventfd.write_u64(1);
        }
    }

    fn wake_cond(&self) -> bool {
        self.is_woken
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    fn host_eventfd(&self) -> &HostEventFd {
        &self.host_eventfd
    }
}

impl LevelSync {
    #[inline(always)]
    fn is_woken(&self) -> bool {
        self.is_woken.load(Ordering::Acquire)
    }
}
