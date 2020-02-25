use super::*;
use crate::syscall::{occlum_syscall, SyscallNum};
use sgx_types::*;

pub const SYSCALL_OPCODE: u16 = 0x050F;

pub fn handle_syscall_exception(info: &mut sgx_exception_info_t) -> u32 {
    debug!("handle SYSCALL exception");
    // SYSCALL, save RIP into RCX and RFLAGS into R11
    info.cpu_context.rcx = info.cpu_context.rip + 2;
    info.cpu_context.r11 = info.cpu_context.rflags;
    let num = info.cpu_context.rax as u32;
    let arg0 = info.cpu_context.rdi as isize;
    let arg1 = info.cpu_context.rsi as isize;
    let arg2 = info.cpu_context.rdx as isize;
    let arg3 = info.cpu_context.r10 as isize;
    let arg4 = info.cpu_context.r8 as isize;
    let arg5 = info.cpu_context.r9 as isize;
    // syscall should not be an exception in Occlum
    assert!(num != SyscallNum::Exception as u32);
    let ret = occlum_syscall(num, arg0, arg1, arg2, arg3, arg4, arg5);
    info.cpu_context.rax = ret as u64;

    // SYSRET, load RIP from RCX and loading RFLAGS from R11
    info.cpu_context.rip = info.cpu_context.rcx;
    // Clear RF, VM, reserved bits; set bit 1
    info.cpu_context.rflags = (info.cpu_context.r11 & 0x3C7FD7) | 2;

    EXCEPTION_CONTINUE_EXECUTION
}
