use super::*;
use crate::syscall::SyscallNum;
use sgx_types::*;

const RDTSC_OPCODE: u16 = 0x310F;

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

    let (low, high) = {
        let mut low = 0;
        let mut high = 0;
        let ret = unsafe { __occlum_syscall(SyscallNum::Rdtsc as u32, &mut low, &mut high) };
        assert!(ret == 0);
        (low, high)
    };
    info.cpu_context.rax = low as u64;
    info.cpu_context.rdx = high as u64;
    info.cpu_context.rip += 2;

    EXCEPTION_CONTINUE_EXECUTION
}

extern "C" {
    fn __occlum_syscall(num: u32, arg0: *mut u32, arg1: *mut u32) -> i64;
}
