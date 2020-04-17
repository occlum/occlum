use super::*;
use crate::syscall::{occlum_syscall, CpuContext, SyscallNum};
use sgx_types::*;

pub const SYSCALL_OPCODE: u16 = 0x050F;

pub fn handle_syscall_exception(user_context: &mut CpuContext) -> ! {
    debug!("handle SYSCALL exception");

    // SYSCALL instruction saves RIP into RCX and RFLAGS into R11. This is to
    // comply with hardware's behavoir. Not useful for us.
    user_context.rcx = user_context.rip;
    user_context.r11 = user_context.rflags;

    // The target RIP should be the next instruction
    user_context.rip += 2;
    // Set target RFLAGS: clear RF, VM, reserved bits; set bit 1
    user_context.rflags = (user_context.rflags & 0x3C7FD7) | 2;

    let num = user_context.rax as u32;
    assert!(num != SyscallNum::HandleException as u32);

    // FIXME: occlum syscall must use Linux ABI
    occlum_syscall(user_context);
}
