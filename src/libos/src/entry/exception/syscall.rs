use super::*;
use crate::entry::context_switch::CURRENT_CONTEXT;
use sgx_types::*;

pub const SYSCALL_OPCODE: u16 = 0x050F;

pub async fn handle_syscall_exception() -> Result<()> {
    debug!("handle SYSCALL exception");

    CURRENT_CONTEXT.with(|_context| {
        let mut context = _context.borrow_mut();
        let gp_regs = &mut context.gp_regs;

        // SYSCALL instruction saves RIP into RCX and RFLAGS into R11. This is to
        // comply with hardware's behavior. Not useful for us.
        gp_regs.rcx = gp_regs.rip;
        gp_regs.r11 = gp_regs.rflags;

        // The target RIP should be the next instruction
        gp_regs.rip += 2;
        // Set target RFLAGS: clear RF, VM, reserved bits; set bit 1
        gp_regs.rflags = (gp_regs.rflags & 0x3C7FD7) | 2;
    });

    crate::entry::syscall::handle_syscall().await
}
