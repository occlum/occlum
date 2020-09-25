pub use self::sgx::sgx_interrupt_info_t;
use crate::prelude::*;
use crate::process::ThreadRef;
use crate::syscall::exception_interrupt_syscall_c_abi;
use crate::syscall::{CpuContext, FpRegs, SyscallNum};
use aligned::{Aligned, A16};
use core::arch::x86_64::_fxsave;

mod sgx;

pub fn init() {
    unsafe {
        let status = sgx::sgx_interrupt_init(handle_interrupt);
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
}

extern "C" fn handle_interrupt(info: *mut sgx_interrupt_info_t) -> i32 {
    let mut fpregs = FpRegs::save();
    unsafe {
        exception_interrupt_syscall_c_abi(
            SyscallNum::HandleInterrupt as u32,
            info as *mut _,
            &mut fpregs as *mut FpRegs,
        )
    };
    unreachable!();
}

pub fn do_handle_interrupt(
    info: *mut sgx_interrupt_info_t,
    fpregs: *mut FpRegs,
    cpu_context: *mut CpuContext,
) -> Result<isize> {
    let info = unsafe { &*info };
    let context = unsafe { &mut *cpu_context };
    // The cpu context is overriden so that it is as if the syscall is called from where the
    // interrupt happened
    *context = CpuContext::from_sgx(&info.cpu_context);
    context.fpregs = fpregs;
    Ok(0)
}

/// Broadcast interrupts to threads by sending POSIX signals.
pub fn broadcast_interrupts() -> Result<usize> {
    let should_interrupt_thread = |thread: &&ThreadRef| -> bool {
        // TODO: check Thread::sig_mask to reduce false positives
        thread.process().is_forced_to_exit()
            || !thread.sig_queues().read().unwrap().empty()
            || !thread.process().sig_queues().read().unwrap().empty()
    };

    let num_signaled_threads = crate::process::table::get_all_threads()
        .iter()
        .filter(should_interrupt_thread)
        .map(|thread| {
            let host_tid = {
                let sched = thread.sched().lock().unwrap();
                match sched.host_tid() {
                    None => return false,
                    Some(host_tid) => host_tid,
                }
            };
            let signum = 64; // real-time signal 64 is used to notify interrupts
            let is_signaled = unsafe {
                let mut retval = 0;
                let status = occlum_ocall_tkill(&mut retval, host_tid, signum);
                assert!(status == sgx_status_t::SGX_SUCCESS);
                if retval == 0 {
                    true
                } else {
                    false
                }
            };
            is_signaled
        })
        .filter(|&is_signaled| is_signaled)
        .count();
    Ok(num_signaled_threads)
}

extern "C" {
    fn occlum_ocall_tkill(retval: &mut i32, host_tid: pid_t, signum: i32) -> sgx_status_t;
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
