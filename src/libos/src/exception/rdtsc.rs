use super::*;
use sgx_types::*;

const RDTSC_OPCODE: u16 = 0x310F;
static mut FAKE_RDTSC_VALUE: u64 = 0;
static FAKE_RDTSC_INC_VALUE: u64 = 1000;

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
    // rdtsc support here is temporary, only for SKL, later CPU's will support this inside enclave
    unsafe {
        FAKE_RDTSC_VALUE += FAKE_RDTSC_INC_VALUE;
        info.cpu_context.rax = (FAKE_RDTSC_VALUE & 0xFFFFFFFF);
        info.cpu_context.rdx = (FAKE_RDTSC_VALUE >> 32);
    }
    info.cpu_context.rip += 2;

    EXCEPTION_CONTINUE_EXECUTION
}
