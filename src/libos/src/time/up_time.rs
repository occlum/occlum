use super::{do_clock_gettime, ClockId};
use std::time::Duration;

lazy_static! {
    static ref BOOT_TIME_STAMP: Duration = do_clock_gettime(ClockId::CLOCK_MONOTONIC_RAW)
        .unwrap()
        .as_duration();
}

pub fn init() {
    *BOOT_TIME_STAMP;
}

pub fn get() -> Option<Duration> {
    do_clock_gettime(ClockId::CLOCK_MONOTONIC_RAW)
        .unwrap()
        .as_duration()
        .checked_sub(*BOOT_TIME_STAMP)
}
