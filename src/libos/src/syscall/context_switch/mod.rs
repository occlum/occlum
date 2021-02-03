use std::cell::RefCell;

mod cpu_context;
mod fp_regs;

pub use self::cpu_context::CpuContext;
pub use self::fp_regs::FpRegs;

async_rt::task_local! {
    pub static CURRENT_CONTEXT: RefCell<CpuContext> = RefCell::new(CpuContext::default());
}

extern "C" {
    /// Switch the execution of the current vCPU to the user space.
    pub fn switch_to_user(user_context: *mut CpuContext);

    /// Get a pointer to the current in-use `CpuContext`, i.e., the pointer that has
    /// been passed to the current on-going `switch_to_user` function.
    ///
    /// If the `switch_to_user` has returned or not called on the current vCPU, then
    /// the return value of this function will be a null pointer.
    pub fn current_context_ptr() -> *mut CpuContext;
}
