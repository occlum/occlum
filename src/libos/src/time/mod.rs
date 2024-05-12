use self::timer_slack::*;
use super::*;
use crate::exception::is_cpu_support_sgx2;
use core::convert::TryFrom;
use process::pid_t;
use rcore_fs::dev::TimeProvider;
use rcore_fs::vfs::Timespec;
use sgx_trts::enclave::{rsgx_get_enclave_mode, EnclaveMode};
use spin::Once;
use std::time::Duration;
use std::{fmt, u64};
use syscall::SyscallNum;

mod profiler;
pub mod timer_slack;
pub mod up_time;

pub use profiler::ThreadProfiler;
pub use timer_slack::TIMERSLACK;
pub use vdso_time::ClockId;

#[allow(non_camel_case_types)]
pub type time_t = i64;

#[allow(non_camel_case_types)]
pub type suseconds_t = i64;

#[allow(non_camel_case_types)]
pub type clock_t = i64;

/// Clock ticks per second
pub const SC_CLK_TCK: u64 = 100;

static IS_ENABLE_VDSO: Once<bool> = Once::new();

pub fn init() {
    init_vdso();
    up_time::init();
}

fn init_vdso() {
    IS_ENABLE_VDSO.call_once(|| match rsgx_get_enclave_mode() {
        EnclaveMode::Hw if is_cpu_support_sgx2() => true,
        EnclaveMode::Sim => true,
        _ => false,
    });
}

#[inline(always)]
fn is_enable_vdso() -> bool {
    IS_ENABLE_VDSO.get().map_or(false, |is_enable| *is_enable)
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct timeval_t {
    sec: time_t,
    usec: suseconds_t,
}

impl timeval_t {
    pub fn new(sec: time_t, usec: suseconds_t) -> Self {
        let time = Self { sec, usec };

        time.validate().unwrap();
        time
    }

    pub fn validate(&self) -> Result<()> {
        if self.sec >= 0 && self.usec >= 0 && self.usec < 1_000_000 {
            Ok(())
        } else {
            return_errno!(EINVAL, "invalid value for timeval_t");
        }
    }

    pub fn sec(&self) -> time_t {
        self.sec
    }

    pub fn usec(&self) -> suseconds_t {
        self.usec
    }

    pub fn as_duration(&self) -> Duration {
        Duration::new(self.sec as u64, (self.usec * 1_000) as u32)
    }
}

impl From<Duration> for timeval_t {
    fn from(duration: Duration) -> timeval_t {
        let sec = duration.as_secs() as time_t;
        let usec = duration.subsec_micros() as i64;
        debug_assert!(sec >= 0); // nsec >= 0 always holds
        timeval_t { sec, usec }
    }
}

pub fn do_gettimeofday() -> timeval_t {
    let duration = if is_enable_vdso() {
        vdso_time::clock_gettime(ClockId::CLOCK_REALTIME).unwrap()
    } else {
        // SGX1 Hardware doesn't support rdtsc instruction
        vdso_time::clock_gettime_slow(ClockId::CLOCK_REALTIME).unwrap()
    };

    let tv = timeval_t::from(duration);
    tv.validate()
        .expect("gettimeofday returned invalid timeval_t");
    tv
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct timespec_t {
    sec: time_t,
    nsec: i64,
}

impl From<Duration> for timespec_t {
    fn from(duration: Duration) -> timespec_t {
        let sec = duration.as_secs() as time_t;
        let nsec = duration.subsec_nanos() as i64;
        debug_assert!(sec >= 0); // nsec >= 0 always holds
        timespec_t { sec, nsec }
    }
}

impl From<timeval_t> for timespec_t {
    fn from(timval: timeval_t) -> timespec_t {
        timespec_t {
            sec: timval.sec(),
            nsec: timval.usec() * 1_000,
        }
    }
}

impl From<time_t> for timespec_t {
    fn from(time: time_t) -> timespec_t {
        timespec_t { sec: time, nsec: 0 }
    }
}

impl timespec_t {
    pub fn from_raw_ptr(ptr: *const timespec_t) -> Result<timespec_t> {
        let ts = unsafe { *ptr };
        ts.validate()?;
        Ok(ts)
    }

    pub fn validate(&self) -> Result<()> {
        if self.sec >= 0 && self.nsec >= 0 && self.nsec < 1_000_000_000 {
            Ok(())
        } else {
            return_errno!(EINVAL, "invalid value for timespec_t");
        }
    }

    pub fn sec(&self) -> time_t {
        self.sec
    }

    pub fn nsec(&self) -> i64 {
        self.nsec
    }

    pub fn as_duration(&self) -> Duration {
        Duration::new(self.sec as u64, self.nsec as u32)
    }
}

#[allow(non_camel_case_types)]
pub type clockid_t = i32;

pub fn do_clock_gettime(clockid: ClockId) -> Result<timespec_t> {
    let duration = if is_enable_vdso() {
        vdso_time::clock_gettime(clockid).unwrap()
    } else {
        // SGX1 Hardware doesn't support rdtsc instruction
        vdso_time::clock_gettime_slow(clockid).unwrap()
    };

    let tv = timespec_t::from(duration);
    tv.validate()
        .expect("clock_gettime returned invalid timespec");
    Ok(tv)
}

pub fn do_clock_getres(clockid: ClockId) -> Result<timespec_t> {
    let duration = if is_enable_vdso() {
        vdso_time::clock_getres(clockid).unwrap()
    } else {
        // SGX1 Hardware doesn't support rdtsc instruction
        vdso_time::clock_getres_slow(clockid).unwrap()
    };

    let res = timespec_t::from(duration);
    let validate_resolution = |res: &timespec_t| -> Result<()> {
        // The resolution can be ranged from 1 nanosecond to a few milliseconds
        if res.sec == 0 && res.nsec > 0 && res.nsec < 1_000_000_000 {
            Ok(())
        } else {
            return_errno!(EINVAL, "invalid value for resolution");
        }
    };
    // do sanity check
    validate_resolution(&res).expect("clock_getres returned invalid resolution");
    Ok(res)
}

const TIMER_ABSTIME: i32 = 0x01;

pub fn do_clock_nanosleep(
    clockid: ClockId,
    flags: i32,
    req: &timespec_t,
    rem: Option<&mut timespec_t>,
) -> Result<()> {
    extern "C" {
        fn occlum_ocall_clock_nanosleep(
            ret: *mut i32,
            clockid: clockid_t,
            flags: i32,
            req: *const timespec_t,
            rem: *mut timespec_t,
        ) -> sgx_status_t;
    }

    let mut ret = 0;
    let mut u_rem: timespec_t = timespec_t { sec: 0, nsec: 0 };
    match clockid {
        ClockId::CLOCK_REALTIME
        | ClockId::CLOCK_MONOTONIC
        | ClockId::CLOCK_BOOTTIME
        | ClockId::CLOCK_PROCESS_CPUTIME_ID => {}
        ClockId::CLOCK_THREAD_CPUTIME_ID => {
            return_errno!(EINVAL, "CLOCK_THREAD_CPUTIME_ID is not a permitted value");
        }
        _ => {
            return_errno!(EOPNOTSUPP, "does not support sleeping against this clockid");
        }
    }
    let sgx_status = unsafe {
        occlum_ocall_clock_nanosleep(&mut ret, clockid as clockid_t, flags, req, &mut u_rem)
    };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
    assert!(ret == 0 || ret == Errno::EINTR as i32);
    if ret != 0 {
        assert!(u_rem.as_duration() <= req.as_duration() + (*TIMERSLACK).to_duration());
        // rem is only valid if TIMER_ABSTIME flag is not set
        if flags != TIMER_ABSTIME {
            if let Some(rem) = rem {
                *rem = u_rem;
            }
        }
        return_errno!(EINTR, "sleep interrupted");
    }
    return Ok(());
}

pub fn do_nanosleep(req: &timespec_t, rem: Option<&mut timespec_t>) -> Result<()> {
    // POSIX.1 specifies that nanosleep() should measure time against
    // the CLOCK_REALTIME clock.  However, Linux measures the time using
    // the CLOCK_MONOTONIC clock.
    // Here we follow the POSIX.1
    let clock_id = ClockId::CLOCK_REALTIME;
    return do_clock_nanosleep(clock_id, 0, req, rem);
}

pub fn do_thread_getcpuclock() -> Result<timespec_t> {
    extern "C" {
        fn occlum_ocall_thread_getcpuclock(ret: *mut c_int, tp: *mut timespec_t) -> sgx_status_t;
    }

    let mut tv: timespec_t = Default::default();
    try_libc!({
        let mut retval: i32 = 0;
        let status = occlum_ocall_thread_getcpuclock(&mut retval, &mut tv as *mut timespec_t);
        assert!(status == sgx_status_t::SGX_SUCCESS);
        retval
    });
    tv.validate()?;
    Ok(tv)
}

pub fn do_rdtsc() -> (u32, u32) {
    extern "C" {
        fn occlum_ocall_rdtsc(low: *mut u32, high: *mut u32) -> sgx_status_t;
    }
    let mut low = 0;
    let mut high = 0;
    let sgx_status = unsafe { occlum_ocall_rdtsc(&mut low, &mut high) };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
    (low, high)
}

// For SEFS
pub struct OcclumTimeProvider;

impl TimeProvider for OcclumTimeProvider {
    fn current_time(&self) -> Timespec {
        let time = do_gettimeofday();
        Timespec {
            sec: time.sec,
            nsec: time.usec * 1000,
        }
    }
}

impl ext2_rs::TimeProvider for OcclumTimeProvider {
    fn now(&self) -> ext2_rs::UnixTime {
        let time = do_gettimeofday();
        ext2_rs::UnixTime { sec: time.sec as _ }
    }
}

// For Timerfd
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct itimerspec_t {
    it_interval: timespec_t,
    it_value: timespec_t,
}

impl itimerspec_t {
    pub fn from_raw_ptr(ptr: *const itimerspec_t) -> Result<itimerspec_t> {
        let its = unsafe { *ptr };
        its.validate()?;
        Ok(its)
    }
    pub fn validate(&self) -> Result<()> {
        self.it_interval.validate()?;
        self.it_value.validate()?;
        Ok(())
    }
}
