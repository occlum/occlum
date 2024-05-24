use crate::fs::IoctlCmd;
use crate::prelude::*;
use libc::{suseconds_t, time_t};
use std::time::Duration;

crate::impl_ioctl_cmd! {
    pub struct SetSendTimeoutCmd<Input=Duration, Output=()> {}
}

crate::impl_ioctl_cmd! {
    pub struct SetRecvTimeoutCmd<Input=Duration, Output=()> {}
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
