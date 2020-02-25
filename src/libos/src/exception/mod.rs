use self::cpuid::{handle_cpuid_exception, setup_cpuid_info, CPUID_OPCODE};
use self::rdtsc::{handle_rdtsc_exception, RDTSC_OPCODE};
use self::syscall::{handle_syscall_exception, SYSCALL_OPCODE};
use super::*;
use crate::syscall::SyscallNum;
use sgx_types::*;

pub fn register_exception_handlers() {
    setup_cpuid_info();
    unsafe {
        sgx_register_exception_handler(1, handle_exception);
    }
}

#[no_mangle]
extern "C" fn handle_exception(info: *mut sgx_exception_info_t) -> u32 {
    let ret = unsafe { __occlum_syscall(SyscallNum::Exception as u32, info) };
    assert!(ret == EXCEPTION_CONTINUE_EXECUTION);
    ret
}

pub fn do_handle_exception(info: *mut sgx_exception_info_t) -> Result<isize> {
    let mut info = unsafe { &mut *info };
    // Assume the length of opcode is 2 bytes
    let ip_opcode = unsafe { *(info.cpu_context.rip as *const u16) };
    if info.exception_vector != sgx_exception_vector_t::SGX_EXCEPTION_VECTOR_UD
        || info.exception_type != sgx_exception_type_t::SGX_EXCEPTION_HARDWARE
    {
        panic!(
            "unable to process the exception, vector:{} type:{}",
            info.exception_vector as u32, info.exception_type as u32
        );
    }
    let ret = match ip_opcode {
        #![deny(unreachable_patterns)]
        CPUID_OPCODE => handle_cpuid_exception(&mut info),
        RDTSC_OPCODE => handle_rdtsc_exception(&mut info),
        SYSCALL_OPCODE => handle_syscall_exception(&mut info),
        _ => panic!("unable to process the exception, opcode: {:#x}", ip_opcode),
    };
    Ok(ret as isize)
}

extern "C" {
    fn __occlum_syscall(num: u32, info: *mut sgx_exception_info_t) -> u32;
}

mod cpuid;
mod rdtsc;
mod syscall;
