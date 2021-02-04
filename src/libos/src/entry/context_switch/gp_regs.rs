use crate::prelude::*;

/// The general-purpose registers of CPU.
///
/// Note. The Rust definition of this struct must be kept in sync with assembly code.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct GpRegs {
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rsp: u64,
    pub rip: u64,
    pub rflags: u64,
}

impl From<&sgx_cpu_context_t> for GpRegs {
    fn from(src: &sgx_cpu_context_t) -> Self {
        Self {
            r8: src.r8,
            r9: src.r9,
            r10: src.r10,
            r11: src.r11,
            r12: src.r12,
            r13: src.r13,
            r14: src.r14,
            r15: src.r15,
            rdi: src.rdi,
            rsi: src.rsi,
            rbp: src.rbp,
            rbx: src.rbx,
            rdx: src.rdx,
            rax: src.rax,
            rcx: src.rcx,
            rsp: src.rsp,
            rip: src.rip,
            rflags: src.rflags,
        }
    }
}
