use crate::events::{Event, EventFilter, Notifier, Observer};
use crate::prelude::*;

bitflags! {
    pub struct IoEvents: u32 {
        const IN = 0x001; // = POLLIN
        const OUT = 0x004; // = POLLOUT
        const PRI = 0x002; // = POLLPRI
        const ERR = 0x008; // = POLLERR
        const RDHUP = 0x2000; // = POLLRDHUP
        const HUP = 0x010; // = POLLHUP
    }
}

impl Event for IoEvents {}

impl EventFilter<IoEvents> for IoEvents {
    fn filter(&self, events: &IoEvents) -> bool {
        self.intersects(*events)
    }
}

pub type IoNotifier = Notifier<IoEvents, IoEvents>;
