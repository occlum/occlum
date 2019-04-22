use rcore_fs::dev::TimeProvider;
use rcore_fs::vfs::Timespec;

use super::*;

#[allow(non_camel_case_types)]
pub type time_t = i64;

#[allow(non_camel_case_types)]
pub type suseconds_t = i64;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
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
