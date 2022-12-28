use super::{FpRegs, GpRegs};
use crate::prelude::*;

/// Cpu context, including both general-purpose registers and floating-point registers.
///
/// Note. The Rust definition of this struct must be kept in sync with assembly code.
#[derive(Clone, Default)]
#[repr(C)]
pub struct CpuContext {
    pub gp_regs: GpRegs,
    pub fs_base: u64,
    pub fp_regs: FpRegs,
}

impl CpuContext {
    pub fn new() -> Self {
        Default::default()
    }
}

impl std::fmt::Debug for CpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuContext")
            .field("gp_regs", &self.gp_regs)
            .field("fs_base", &self.fs_base)
            .field("fp_regs", &"<omitted>")
            .finish()
    }
}
