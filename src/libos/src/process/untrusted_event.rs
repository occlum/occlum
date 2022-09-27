use crate::prelude::*;

pub(crate) fn wait_event(thread: *const c_void) {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_wait_untrusted_event_ocall(&mut ret as *mut c_int, thread);
    }
    if ret != 0 || sgx_ret != 0 {
        panic!("ERROR: OCall failed!");
    }
}

pub(crate) fn set_event(thread: *const c_void) {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_set_untrusted_event_ocall(&mut ret as *mut c_int, thread);
    }
    if ret != 0 || sgx_ret != 0 {
        panic!("ERROR: OCall failed!");
    }
}

extern "C" {
    /* Go outside and wait on my untrusted event */
    pub(crate) fn sgx_thread_wait_untrusted_event_ocall(
        ret: *mut c_int,
        self_thread: *const c_void,
    ) -> c_int;

    /* Wake a thread waiting on its untrusted event */
    pub(crate) fn sgx_thread_set_untrusted_event_ocall(
        ret: *mut c_int,
        waiter_thread: *const c_void,
    ) -> c_int;

    /* Wake a thread waiting on its untrusted event, and wait on my untrusted event */
    pub(crate) fn sgx_thread_setwait_untrusted_events_ocall(
        ret: *mut c_int,
        waiter_thread: *const c_void,
        self_thread: *const c_void,
    ) -> c_int;

    /* Wake multiple threads waiting on their untrusted events */
    pub(crate) fn sgx_thread_set_multiple_untrusted_events_ocall(
        ret: *mut c_int,
        waiter_threads: *const *const c_void,
        total: size_t,
    ) -> c_int;
}
