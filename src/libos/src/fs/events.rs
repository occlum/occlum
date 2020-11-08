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

impl EventFilter<IoEvents> for IoEvents {
    fn filter(&self, events: &IoEvents) -> bool {
        self.intersects(*events)
    }
}

pub type IoNotifier = Notifier<IoEvents, IoEvents>;
