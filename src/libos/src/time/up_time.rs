use super::{do_clock_gettime, ClockId};
use std::time::Duration;

lazy_static! {
    static ref BOOT_TIME_STAMP: Duration = do_clock_gettime(ClockId::CLOCK_MONOTONIC_RAW)
        .unwrap()
        .as_duration();
    static ref BOOT_TIME_STAMP_SINCE_EPOCH: Duration = do_clock_gettime(ClockId::CLOCK_REALTIME)
        .unwrap()
        .as_duration();
}

pub(super) fn init() {
    *BOOT_TIME_STAMP;
    *BOOT_TIME_STAMP_SINCE_EPOCH;
}

pub fn boot_time_since_epoch() -> Duration {
    *BOOT_TIME_STAMP_SINCE_EPOCH
}

pub fn get() -> Option<Duration> {
    do_clock_gettime(ClockId::CLOCK_MONOTONIC_RAW)
        .unwrap()
        .as_duration()
        .checked_sub(*BOOT_TIME_STAMP)
}
