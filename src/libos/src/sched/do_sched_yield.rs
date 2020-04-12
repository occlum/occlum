use crate::prelude::*;

pub fn do_sched_yield() {
    extern "C" {
        fn occlum_ocall_sched_yield() -> sgx_status_t;
    }
    unsafe {
        let status = occlum_ocall_sched_yield();
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
}
