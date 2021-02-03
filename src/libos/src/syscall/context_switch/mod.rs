use std::cell::RefCell;

mod cpu_context;
mod fp_regs;

pub use self::cpu_context::CpuContext;
pub use self::fp_regs::FpRegs;

async_rt::task_local! {
    pub static CURRENT_CONTEXT: RefCell<CpuContext> = RefCell::new(CpuContext::default());
}
