use crate::ioctl::IoctlCmd;
use crate::prelude::*;
use libc::{suseconds_t, time_t};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Timeout {
    sender: Option<Duration>,
    receiver: Option<Duration>,
}

impl Timeout {
    pub fn new() -> Self {
        Self {
            sender: None,
            receiver: None,
        }
    }

    pub fn sender_timeout(&self) -> Option<Duration> {
        self.sender
    }

    pub fn receiver_timeout(&self) -> Option<Duration> {
        self.receiver
    }

    pub fn set_sender(&mut self, timeout: Duration) {
        self.sender = Some(timeout);
    }

    pub fn set_receiver(&mut self, timeout: Duration) {
        self.receiver = Some(timeout);
    }
}

#[derive(Debug)]
pub struct SetSendTimeoutCmd(Duration);

impl IoctlCmd for SetSendTimeoutCmd {}

impl SetSendTimeoutCmd {
    pub fn new(timeout: Duration) -> Self {
        Self(timeout)
    }

    pub fn timeout(&self) -> &Duration {
        &self.0
    }
}

#[derive(Debug)]
pub struct SetRecvTimeoutCmd(Duration);

impl IoctlCmd for SetRecvTimeoutCmd {}

impl SetRecvTimeoutCmd {
    pub fn new(timeout: Duration) -> Self {
        Self(timeout)
    }

    pub fn timeout(&self) -> &Duration {
        &self.0
    }
}

crate::impl_ioctl_cmd! {
    pub struct GetSendTimeoutCmd<Input=(), Output=timeval> {}
}

crate::impl_ioctl_cmd! {
    pub struct GetRecvTimeoutCmd<Input=(), Output=timeval> {}
}

pub fn timeout_to_timeval(timeout: Option<Duration>) -> timeval {
    match timeout {
        Some(duration) => {
            let sec = duration.as_secs();
            let usec = duration.subsec_micros();
            timeval {
                sec: sec as time_t,
                usec: usec as suseconds_t,
            }
        }
        None => timeval { sec: 0, usec: 0 },
    }
}

// Same as libc::timeval
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct timeval {
    sec: time_t,
    usec: suseconds_t,
}
