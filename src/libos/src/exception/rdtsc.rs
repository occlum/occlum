use super::*;
use process::Task;
use sgx_types::*;

const RDTSC_OPCODE: u16 = 0x310F;

extern "C" {
    fn occlum_ocall_rdtsc(low: *mut u32, high: *mut u32) -> sgx_status_t;
    fn __get_current_task() -> *const Task;
    fn switch_td_to_kernel(task: *const Task);
    fn switch_td_to_user(task: *const Task);
}

#[no_mangle]
pub extern "C" fn handle_rdtsc_exception(info: *mut sgx_exception_info_t) -> u32 {
    let info = unsafe { &mut *info };
    let ip_opcode = unsafe { *(info.cpu_context.rip as *const u16) };
    if info.exception_vector != sgx_exception_vector_t::SGX_EXCEPTION_VECTOR_UD
        || info.exception_type != sgx_exception_type_t::SGX_EXCEPTION_HARDWARE
        || ip_opcode != RDTSC_OPCODE
    {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    unsafe {
        let task = __get_current_task();
        switch_td_to_kernel(task);
        let (low, high) = {
            let mut low = 0;
            let mut high = 0;
            let sgx_status = occlum_ocall_rdtsc(&mut low, &mut high);
            assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
            (low, high)
        };
        info.cpu_context.rax = low as u64;
        info.cpu_context.rdx = high as u64;
        switch_td_to_user(task);
    }
    info.cpu_context.rip += 2;

    EXCEPTION_CONTINUE_EXECUTION
}
