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
}

pub fn do_gettimeofday() -> timeval_t {
    let mut tv: timeval_t = Default::default();
    unsafe {
        ocall_gettimeofday(&mut tv.sec as *mut time_t, &mut tv.usec as *mut suseconds_t);
    }
    tv
}

extern "C" {
    fn ocall_gettimeofday(sec: *mut time_t, usec: *mut suseconds_t) -> sgx_status_t;
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct timespec_t {
    sec: time_t,
    nsec: i64,
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
    pub fn from_raw(clockid: clockid_t) -> Result<ClockID, Error> {
        Ok(match clockid as i32 {
            0 => ClockID::CLOCK_REALTIME,
            1 => ClockID::CLOCK_MONOTONIC,
            2 => ClockID::CLOCK_PROCESS_CPUTIME_ID,
            3 => ClockID::CLOCK_THREAD_CPUTIME_ID,
            4 => ClockID::CLOCK_MONOTONIC_RAW,
            5 => ClockID::CLOCK_REALTIME_COARSE,
            6 => ClockID::CLOCK_MONOTONIC_COARSE,
            7 => ClockID::CLOCK_BOOTTIME,
            _ => return errno!(EINVAL, "invalid command"),
        })
    }
}

pub fn do_clock_gettime(clockid: ClockID) -> Result<timespec_t, Error> {
    let mut sec = 0;
    let mut nsec = 0;
    unsafe {
        ocall_clock_gettime(
            clockid as clockid_t,
            &mut sec as *mut time_t,
            &mut nsec as *mut i64,
        );
    }
    Ok(timespec_t { sec, nsec })
}

extern "C" {
    fn ocall_clock_gettime(clockid: clockid_t, sec: *mut time_t, ns: *mut i64) -> sgx_status_t;
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
