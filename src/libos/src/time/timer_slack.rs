use super::*;
use std::time::Duration;

pub struct TimerSlack {
    nanoseconds: u32,
}

impl TimerSlack {
    pub fn new(nanoseconds: u32) -> Result<Self> {
        let timerslack = Self { nanoseconds };
        timerslack.validate()?;
        Ok(timerslack)
    }

    pub fn validate(&self) -> Result<()> {
        // Timer slack bigger than 1ms is considered invalid here. The kernel default timer slack is 50us.
        if self.nanoseconds < 1_000_000 {
            Ok(())
        } else {
            return_errno!(EINVAL, "invalid value for TimerSlack");
        }
    }

    pub fn to_u32(&self) -> u32 {
        self.nanoseconds
    }

    pub fn to_duration(&self) -> Duration {
        Duration::from_nanos(self.to_u32() as u64)
    }
}

lazy_static! {
    pub static ref TIMERSLACK: TimerSlack =
        do_get_timerslack().unwrap_or(TimerSlack::new(50_000).unwrap()); // Use kernel default timer slack 50us.
}

fn do_get_timerslack() -> Result<TimerSlack> {
    extern "C" {
        fn occlum_ocall_get_timerslack(nanosecond: *mut i32) -> sgx_status_t;
    }
    let mut timer_slack: i32 = 0;
    let sgx_status = unsafe { occlum_ocall_get_timerslack(&mut timer_slack as *mut i32) };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS);

    TimerSlack::new(timer_slack as u32)
}
