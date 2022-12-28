use core::arch::x86_64::_fxsave;

use aligned::{Aligned, A16};

use self::sgx::sgx_interrupt_info_t;
use super::context_switch::{self, GpRegs};
use crate::prelude::*;
use crate::process::ThreadRef;

mod sgx;

pub fn init() {
    unsafe {
        let status = sgx::sgx_interrupt_init(interrupt_entrypoint);
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
}

extern "C" fn interrupt_entrypoint(sgx_interrupt_info: *mut sgx_interrupt_info_t) -> i32 {
    let sgx_interrupt_info = unsafe { &mut *sgx_interrupt_info };

    // Update the current CPU context
    let mut curr_context_ptr = context_switch::current_context_ptr();
    let curr_context = unsafe { curr_context_ptr.as_mut() };
    // Save CPU's floating-point registers at the time when the exception occurs.
    // Note that we do this at the earliest possible time in hope that
    // the floating-point registers have not been tainted by the LibOS.
    curr_context.fp_regs.save();
    // Save CPU's general-purpose registers
    curr_context.gp_regs = GpRegs::from(&sgx_interrupt_info.cpu_context);

    unsafe {
        context_switch::switch_to_kernel_for_interrupt();
    }

    unreachable!("enter_kernel_for_interrupt never returns!");
}

pub async fn handle_interrupt() -> Result<()> {
    debug!("handle interrupt");
    // We use the interrupt as a chance to do preemptive scheduling
    async_rt::scheduler::yield_now().await;
    Ok(())
}

pub fn enable_current_thread() {
    // Interruptible range
    let (addr, size) = {
        let thread = current!();
        let vm = thread.vm();
        let range = vm.get_process_range();
        (range.start(), range.size())
    };
    unsafe {
        let status = sgx::sgx_interrupt_enable(addr, size);
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
}

pub fn disable_current_thread() {
    unsafe {
        let status = sgx::sgx_interrupt_disable();
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
}
