use std::ptr;

use super::FpRegs;
use crate::prelude::*;

/// Cpu context.
///
/// Note. The definition of this struct must be kept in sync with the assembly
/// code in `syscall_entry_x86-64.S`.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CpuContext {
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
    pub fsbase: u64,
    pub fpregs: *mut FpRegs,
}

impl CpuContext {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_sgx(src: &sgx_cpu_context_t) -> CpuContext {
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
            fsbase: 0,
            fpregs: ptr::null_mut(),
        }
    }
}

impl Default for CpuContext {
    fn default() -> Self {
        Self {
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rdi: 0,
            rsi: 0,
            rbp: 0,
            rbx: 0,
            rdx: 0,
            rax: 0,
            rcx: 0,
            rsp: 0,
            rip: 0,
            rflags: 0,
            fsbase: 0,
            fpregs: std::ptr::null_mut(),
        }
    }
}

unsafe impl Send for CpuContext {}
