use rcore_fs::dev::TimeProvider;
use rcore_fs::vfs::Timespec;

use super::*;

#[allow(non_camel_case_types)]
pub type time_t = i64;

#[allow(non_camel_case_types)]
pub type suseconds_t = i64;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct timeval_t {
    sec: time_t,
    usec: suseconds_t,
}

impl timeval_t {
    pub fn as_usec(&self) -> usize {
        (self.sec * 1000000 + self.usec) as usize
    }

    pub fn validate(&self) -> Result<()> {
        if self.sec >= 0 && self.usec >= 0 && self.usec < 1_000_000 {
            Ok(())
        } else {
            return_errno!(EINVAL, "invalid value for timeval_t");
        }
    }
}

pub fn do_gettimeofday() -> timeval_t {
    extern "C" {
        fn occlum_ocall_gettimeofday(tv: *mut timeval_t) -> sgx_status_t;
    }

    let mut tv: timeval_t = Default::default();
    unsafe {
        occlum_ocall_gettimeofday(&mut tv as *mut timeval_t);
    }
    tv.validate().expect("ocall returned invalid timeval_t");
    tv
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct timespec_t {
    sec: time_t,
    nsec: i64,
}

impl timespec_t {
    pub fn from_raw_ptr(ptr: *const timespec_t) -> Result<timespec_t> {
        let ts = unsafe { *ptr };
        if ts.sec < 0 || ts.nsec < 0 || ts.nsec >= 1_000_000_000 {
            return_errno!(EINVAL, "Invalid timespec fields");
        }
        Ok(ts)
    }

    pub fn validate(&self) -> Result<()> {
        if self.sec >= 0 && self.nsec >= 0 && self.nsec < 1_000_000_000 {
            Ok(())
        } else {
            return_errno!(EINVAL, "invalid value for timespec_t");
        }
    }
}

#[allow(non_camel_case_types)]
pub type clockid_t = i32;

#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum ClockID {
    CLOCK_REALTIME = 0,
    CLOCK_MONOTONIC = 1,
    CLOCK_PROCESS_CPUTIME_ID = 2,
    CLOCK_THREAD_CPUTIME_ID = 3,
    CLOCK_MONOTONIC_RAW = 4,
    CLOCK_REALTIME_COARSE = 5,
    CLOCK_MONOTONIC_COARSE = 6,
    CLOCK_BOOTTIME = 7,
}

impl ClockID {
    #[deny(unreachable_patterns)]
    pub fn from_raw(clockid: clockid_t) -> Result<ClockID> {
        Ok(match clockid as i32 {
            0 => ClockID::CLOCK_REALTIME,
            1 => ClockID::CLOCK_MONOTONIC,
            2 => ClockID::CLOCK_PROCESS_CPUTIME_ID,
            3 => ClockID::CLOCK_THREAD_CPUTIME_ID,
            4 => ClockID::CLOCK_MONOTONIC_RAW,
            5 => ClockID::CLOCK_REALTIME_COARSE,
            6 => ClockID::CLOCK_MONOTONIC_COARSE,
            7 => ClockID::CLOCK_BOOTTIME,
            _ => return_errno!(EINVAL, "invalid command"),
        })
    }
}

pub fn do_clock_gettime(clockid: ClockID) -> Result<timespec_t> {
    extern "C" {
        fn occlum_ocall_clock_gettime(clockid: clockid_t, tp: *mut timespec_t) -> sgx_status_t;
    }

    let mut tv: timespec_t = Default::default();
    unsafe {
        occlum_ocall_clock_gettime(clockid as clockid_t, &mut tv as *mut timespec_t);
    }
    tv.validate().expect("ocall returned invalid timespec");
    Ok(tv)
}

pub fn do_nanosleep(req: &timespec_t) -> Result<()> {
    extern "C" {
        fn occlum_ocall_nanosleep(req: *const timespec_t) -> sgx_status_t;
    }
    unsafe {
        occlum_ocall_nanosleep(req as *const timespec_t);
    }
    Ok(())
}

// For SEFS
pub struct OcclumTimeProvider;

impl TimeProvider for OcclumTimeProvider {
    fn current_time(&self) -> Timespec {
        let time = do_gettimeofday();
        Timespec {
            sec: time.sec,
            nsec: time.usec as i32 * 1000,
        }
    }
}
