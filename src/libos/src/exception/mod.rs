//! Exception handling subsystem.

use self::cpuid::{handle_cpuid_exception, setup_cpuid_info, CPUID_OPCODE};
use self::rdtsc::{handle_rdtsc_exception, RDTSC_OPCODE};
use self::syscall::{handle_syscall_exception, SYSCALL_OPCODE};
use super::*;
use crate::signal::{FaultSignal, SigSet};
use crate::syscall::exception_interrupt_syscall_c_abi;
use crate::syscall::{CpuContext, ExtraContext, SyscallNum};
use crate::vm::{enclave_page_fault_handler, is_page_committed, VMRange, USER_SPACE_VM_MANAGER};
use sgx_types::*;
use sgx_types::{sgx_exception_type_t, sgx_exception_vector_t};

pub use self::cpuid::{get_cpuid_info, is_cpu_support_sgx2};

const ENCLU: u32 = 0xd7010f;
const EACCEPT: u32 = 0x5;
const EACCEPTCOPY: u32 = 0x7;

// Modules for instruction simulation
mod cpuid;
mod rdtsc;
mod syscall;

pub fn register_exception_handlers() {
    extern "C" {
        fn sgx_register_exception_handler_for_occlum_user_space(
            user_space_ranges: *const [VMRange; 2],
            handler: sgx_exception_handler_t,
        ) -> sgx_status_t;
    }
    setup_cpuid_info();

    let user_space_ranges: [VMRange; 2] = USER_SPACE_VM_MANAGER.get_user_space_ranges();
    let ret = unsafe {
        sgx_register_exception_handler_for_occlum_user_space(
            &user_space_ranges as *const _,
            handle_exception,
        )
    };
    assert!(ret == sgx_status_t::SGX_SUCCESS);
}

fn try_handle_kernel_exception(info: &sgx_exception_info_t) -> i32 {
    if info.exception_vector == sgx_exception_vector_t::SGX_EXCEPTION_VECTOR_PF {
        let pf_addr = info.exinfo.faulting_address as usize;
        // The PF address must be in the user space. Otherwise, keep searching for the exception handler
        if !USER_SPACE_VM_MANAGER.range().contains(pf_addr) {
            SGX_MM_EXCEPTION_CONTINUE_SEARCH
        } else {
            let rip = info.cpu_context.rip as *const u32;
            let rax = info.cpu_context.rax as u32;
            // This can happen when two threads both try to EAUG a new page. Thread 1 EAUG because it first
            // touches the memory and triggers #PF. Thread 2 EAUG because it uses sgx_mm_commit to commit a
            // new page with EACCEPT and triggers #PF. If Thread 1 first acquires the lock to do EAUG, when Thread 2
            // acquires the lock, it can't do EAUG again and will fail. The failure will raise a signal.
            // This signal will eventually be handled here. And the instruction that triggers this exception is EACCEPT/EACCEPTCOPY.
            // In this case, since the new page is EAUG-ed already, just need to excecute the EACCEPT again. Thus here
            // just return SGX_MM_EXCEPTION_CONTINUE_EXECUTION
            if ENCLU == (unsafe { *rip } as u32) & 0xffffff
                && (EACCEPT == rax || EACCEPTCOPY == rax)
            {
                return SGX_MM_EXCEPTION_CONTINUE_EXECUTION;
            }

            // Check spurious #PF
            // FIXME: We can re-consider this check when we know the root cause
            if is_page_committed(pf_addr) {
                return SGX_MM_EXCEPTION_CONTINUE_EXECUTION;
            }

            // If the triggered code is not user's code and the #PF address is in the userspace, then it is a
            // kernel-triggered #PF that we can handle. This can happen e.g. when read syscall triggers user buffer #PF
            info!("kernel code triggers #PF");
            let kernel_triggers = true;
            enclave_page_fault_handler(info.cpu_context.rip as usize, info.exinfo, kernel_triggers)
                .expect("handle PF failure");
            SGX_MM_EXCEPTION_CONTINUE_EXECUTION
        }
    } else {
        // Otherwise, we can't handle. Keep searching for the exception handler
        error!(
            "We can't handle this exception: {:?}",
            info.exception_vector
        );
        SGX_MM_EXCEPTION_CONTINUE_SEARCH
    }
}

#[no_mangle]
extern "C" fn handle_exception(info: *mut sgx_exception_info_t) -> i32 {
    let info = unsafe { &mut *info };

    // Try handle kernel-trigged #PF
    if !USER_SPACE_VM_MANAGER
        .range()
        .contains(info.cpu_context.rip as usize)
    {
        return try_handle_kernel_exception(&info);
    }

    // User-space-triggered exception
    unsafe {
        exception_interrupt_syscall_c_abi(
            SyscallNum::HandleException as u32,
            info as *mut sgx_exception_info_t as *mut _,
        )
    };
    unreachable!();
}

/// Exceptions are handled as a special kind of system calls.
pub fn do_handle_exception(
    info: *mut sgx_exception_info_t,
    user_context: *mut CpuContext,
) -> Result<isize> {
    let info = unsafe { &mut *info };
    check_exception_type(info.exception_type)?;
    info!("do handle exception: {:?}", info.exception_vector);

    let user_context = unsafe { &mut *user_context };
    *user_context = CpuContext::from_sgx(&info.cpu_context);
    let xsave_area = info.xsave_area.as_mut_ptr();
    user_context.extra_context = ExtraContext::XsaveOnStack;
    user_context.extra_context_ptr = xsave_area;
    user_context.extra_context_size = info.xsave_size;

    // Try to do instruction emulation first
    if info.exception_vector == sgx_exception_vector_t::SGX_EXCEPTION_VECTOR_UD {
        // Assume the length of opcode is 2 bytes
        let ip_opcode: u16 = unsafe { *(user_context.rip as *const u16) };
        if ip_opcode == RDTSC_OPCODE {
            return handle_rdtsc_exception(user_context);
        } else if ip_opcode == SYSCALL_OPCODE {
            return handle_syscall_exception(user_context);
        } else if ip_opcode == CPUID_OPCODE {
            return handle_cpuid_exception(user_context);
        }
    }

    // Normally, We should only handled PF exception with SGX bit set which is due to uncommitted EPC.
    // However, it happens that when committing a no-read-write page (e.g. RWX), there is a short gap
    // after EACCEPTCOPY and before the mprotect ocall. And if the user touches memory during this short
    // gap, the SGX bit will not be set. Thus, here we don't check the SGX bit.
    if info.exception_vector == sgx_exception_vector_t::SGX_EXCEPTION_VECTOR_PF {
        info!("Userspace #PF caught, try handle");
        if enclave_page_fault_handler(info.cpu_context.rip as usize, info.exinfo, false).is_ok() {
            info!("#PF handling is done successfully");
            return Ok(0);
        }

        error!(
            "#PF not handled. Turn to signal. user context = {:?}",
            user_context
        );
    }

    // Then, it must be a "real" exception. Convert it to signal and force delivering it.
    // The generated signal is SIGBUS, SIGFPE, SIGILL, or SIGSEGV.
    //
    // So what happens if the signal is masked? The man page of sigprocmask(2) states:
    //
    // > If SIGBUS, SIGFPE, SIGILL, or SIGSEGV are generated while they are blocked, the result is
    // undefined, unless the signal was generated by kill(2), sigqueue(3), or raise(3).
    //
    // As the thread cannot proceed without handling the exception, we choose to force
    // delivering the signal regardless of the current signal mask.
    let signal = Box::new(FaultSignal::new(info));
    crate::signal::force_signal(signal, user_context);

    Ok(0)
}

// Notes about #PF and #GP exception simulation for SGX 1.
//
// SGX 1 cannot capture #PF and #GP exceptions inside enclaves. This leaves us
// no choice but to rely on untrusted info about #PF or #PG exceptions from
// outside the enclave. Due to the obvious security risk, the feature can be
// disabled.
//
// On the bright side, SGX 2 has native support for #PF and #GP exceptions. So
// this exception simulation and its security risk is not a problem in the long
// run.

#[cfg(not(feature = "sgx1_exception_sim"))]
fn check_exception_type(type_: sgx_exception_type_t) -> Result<()> {
    if type_ != sgx_exception_type_t::SGX_EXCEPTION_HARDWARE {
        return_errno!(EINVAL, "Can only handle hardware exceptions");
    }
    Ok(())
}

#[cfg(feature = "sgx1_exception_sim")]
fn check_exception_type(type_: sgx_exception_type_t) -> Result<()> {
    if type_ != sgx_exception_type_t::SGX_EXCEPTION_HARDWARE
        && type_ != sgx_exception_type_t::SGX_EXCEPTION_SIMULATED
    {
        return_errno!(EINVAL, "Can only handle hardware / simulated exceptions");
    }
    Ok(())
}

// Based on Page-Fault Error Code of Intel Mannul
const PF_EXCEPTION_SGX_BIT: u32 = 0x1;
const PF_EXCEPTION_RW_BIT: u32 = 0x2;

// Return value:
// True     - SGX bit is set
// False    - SGX bit is not set
pub fn check_sgx_bit(exception_error_code: u32) -> bool {
    exception_error_code & PF_EXCEPTION_SGX_BIT == PF_EXCEPTION_SGX_BIT
}

// Return value:
// True     - write bit is set, #PF caused by write
// False    - read bit is set, #PF caused by read
pub fn check_rw_bit(exception_error_code: u32) -> bool {
    exception_error_code & PF_EXCEPTION_RW_BIT == PF_EXCEPTION_RW_BIT
}
