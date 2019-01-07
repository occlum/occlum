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

pub fn do_gettimeofday() -> timeval_t {
    let mut tv : timeval_t = Default::default();
    unsafe {
        ocall_gettimeofday(&mut tv.sec as *mut time_t,
                           &mut tv.usec as *mut suseconds_t);
    }
    tv
}

extern {
    fn ocall_gettimeofday(sec: *mut time_t, usec: *mut suseconds_t) -> sgx_status_t;
}
