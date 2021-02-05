use std::cell::RefCell;
use std::ptr::NonNull;

use crate::entry::exception::Exception;
use crate::prelude::*;

mod cpu_context;
mod fault;
mod fp_regs;
mod gp_regs;

pub use self::cpu_context::CpuContext;
pub use self::fault::Fault;
pub use self::fp_regs::FpRegs;
pub use self::gp_regs::GpRegs;

async_rt::task_local! {
    pub static CURRENT_CONTEXT: RefCell<CpuContext> = RefCell::new(CpuContext::default());
}

/// Switch to the user space according to the content of `CURRENT_CONTEXT`.
///
/// Safety. The content of `CURRENT_CONTEXT` must be valid.
pub unsafe fn switch_to_user() -> Fault {
    let context_ptr = CURRENT_CONTEXT.with(|_context| {
        // Restore user's floating-point state first. Note that there is an implicit
        // assumption that the subsequent LibOS code would not modify floating-
        // point state.
        let mut context = _context.borrow_mut();
        let fp_regs = &mut context.fp_regs;
        if fp_regs.is_valid() {
            fp_regs.restore();
        }
        // After restoring, the content of fp_regs is useless.
        fp_regs.clear();

        _context.as_ptr()
    });

    let mut fault = Fault::Syscall;
    let fault_ptr = &mut fault;

    crate::entry::interrupt::enable_current_thread();
    unsafe {
        _switch_to_user(context_ptr, fault_ptr);
    }
    crate::entry::interrupt::disable_current_thread();

    // Give the compiler a (maybe-useless-but-absolutely-harmless) hint that
    // fault may be updated somewhere else.
    //
    // By default, the resulting fault is Fault::Syscall. However, if the
    // _switch_to_user's return is caused by switch_to_kernel_for_exception or
    // switch_to_kernel_for_interrupt, then the resulting fault will be
    // Fault::Exception(_) or Fault::Interrupt.
    let fault = std::hint::black_box(fault);

    fault
}

/// Switch to kernel space from SGX exception handler.
///
/// Requirement. Since this function causes `switch_to_user` to return, the user
/// must ensure that there is an on-going `switch_to_user` on the current vCPU.
///
/// Furthermore, the caller should provide the CPU state when the exception
/// occurs by updating the CPU context referenced by `current_context_ptr`.
pub unsafe fn switch_to_kernel_for_exception(exception: Exception) -> ! {
    // Limitation. Since this function is intended to be called from SGX exception
    // handler, it must be implemented with minimal runtime assumption. For example,
    // it cannot use Rust's TLS. And it cannot use too much stack.

    let mut fault_ptr = {
        let fault_ptr = __current_fault_ptr();
        NonNull::new(fault_ptr).unwrap()
    };
    let fault = fault_ptr.as_mut();
    *fault = Fault::Exception(exception);

    __switch_to_kernel();
}

/// Switch to kernel space from SGX interrupt handler.
///
/// Similar to the exception version.
pub unsafe fn switch_to_kernel_for_interrupt() -> ! {
    let mut fault_ptr = {
        let fault_ptr = __current_fault_ptr();
        NonNull::new(fault_ptr).unwrap()
    };
    let fault = fault_ptr.as_mut();
    *fault = Fault::Interrupt;

    __switch_to_kernel();
}

/// Get a pointer to the current `CpuContext` that is being used by the
/// on-going `switch_to_user` on the current vCPU.
pub fn current_context_ptr() -> NonNull<CpuContext> {
    let ptr = unsafe { __current_context_ptr() };
    NonNull::new(ptr).unwrap()
}

extern "C" {
    // C functions
    #[allow(improper_ctypes)]
    fn _switch_to_user(user_context: *mut CpuContext, fault: *mut Fault);

    // Assembly functions
    fn __switch_to_kernel() -> !;
    fn __current_context_ptr() -> *mut CpuContext;
    fn __current_fault_ptr() -> *mut Fault;
}
