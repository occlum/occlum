use super::TermStatus;
use sgx_trts::trts::rsgx_raw_is_outside_enclave;
use std::ptr::NonNull;

use crate::prelude::*;

/// A waker to wake up a host thread that waits on the status changes of a LibOS
/// process.
#[derive(Debug)]
pub struct HostWaker {
    ptr: NonNull<i32>,
}

unsafe impl Send for HostWaker {}
unsafe impl Sync for HostWaker {}

impl HostWaker {
    pub fn new(ptr: *mut i32) -> Result<Self> {
        if ptr == std::ptr::null_mut() {
            return_errno!(EINVAL, "the host wake-up pointer must NOT be null");
        }
        let is_outside = rsgx_raw_is_outside_enclave(ptr as *mut u8, std::mem::size_of::<i32>());
        if !is_outside {
            return_errno!(
                EINVAL,
                "the host wake-up pointer must be outside the enclave"
            );
        }
        let ptr = unsafe { NonNull::new_unchecked(ptr) };
        Ok(Self { ptr })
    }

    pub fn wake(&self, term_status: TermStatus) {
        unsafe {
            *&mut *self.ptr.as_ptr() = term_status.as_u32() as i32;

            let sgx_status = occlum_ocall_futex_wake(self.ptr.as_ptr(), i32::max_value());
            assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
        }
    }
}

extern "C" {
    fn occlum_ocall_futex_wake(addr: *mut i32, max_count: i32) -> sgx_status_t;
}
