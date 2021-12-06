use atomic::{Atomic, Ordering};

use crate::events::{Event, EventFilter, Notifier, Observer};
use crate::prelude::*;

bitflags! {
    /// I/O Events
    #[rustfmt::skip]
    pub struct IoEvents: u32 {
        const IN    = 0x0001; // = POLLIN
        const PRI   = 0x0002; // = POLLPRI
        const OUT   = 0x0004; // = POLLOUT
        const ERR   = 0x0008; // = POLLERR
        const HUP   = 0x0010; // = POLLHUP
        const NVAL  = 0x0020; // = POLLNVAL
        const RDHUP = 0x2000; // = POLLRDHUP
    }
}

impl IoEvents {
    pub fn from_raw(raw: u32) -> Self {
        if Self::contains_unrecognizable_bits(raw) {
            warn!("contain unknow flags: {:#x}", raw);
        }
        Self::from_bits_truncate(raw)
    }

    pub fn from_poll_status(poll_status: &crate::rcore_fs::vfs::PollStatus) -> Self {
        if poll_status.error {
            return Self::ERR;
        }
        let mut events = Self::empty();
        if poll_status.read {
            events |= Self::IN
        }
        if poll_status.write {
            events |= Self::OUT
        }
        events
    }

    fn contains_unrecognizable_bits(raw: u32) -> bool {
        // Help to detect four valid but mostly useless flags that we do not
        // handle, yet: POLLRDNORM, POLLRDBAND, POLLWRNORM, annd POLLWRBAND.

        let all_raw = Self::all().to_raw();
        (raw & !all_raw) != 0
    }

    pub fn to_raw(&self) -> u32 {
        self.bits()
    }
}

impl Event for IoEvents {}

pub trait AtomicIoEvents {
    /// Update the IoEvents atomically.
    ///
    /// The update is equivalent to the following assignment
    /// ```
    /// self.store(self.load(Ordering::Relaxed) & !*mask | *ready, ordering)
    /// ```
    fn update(&self, ready: &IoEvents, mask: &IoEvents, ordering: Ordering);
}

impl AtomicIoEvents for Atomic<IoEvents> {
    fn update(&self, ready: &IoEvents, mask: &IoEvents, ordering: Ordering) {
        loop {
            let old_val = self.load(Ordering::Relaxed);
            let new_val = old_val & !*mask | *ready;
            let success_ordering = ordering;
            let failure_ordering = Ordering::Relaxed;
            if self
                .compare_exchange(old_val, new_val, success_ordering, failure_ordering)
                .is_ok()
            {
                return;
            }
        }
    }
}

impl EventFilter<IoEvents> for IoEvents {
    fn filter(&self, events: &IoEvents) -> bool {
        self.intersects(*events)
    }
}

pub type IoNotifier = Notifier<IoEvents, IoEvents>;
