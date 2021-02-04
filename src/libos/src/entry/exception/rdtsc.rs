use super::CURRENT_CONTEXT;
use crate::prelude::*;

pub const RDTSC_OPCODE: u16 = 0x310F;

pub fn handle_rdtsc_exception() -> Result<()> {
    debug!("handle RDTSC exception");
    let (low, high) = crate::time::do_rdtsc();
    trace!("do_rdtsc result {{ low: {:#x} high: {:#x}}}", low, high);
    CURRENT_CONTEXT.with(|_context| {
        let mut context = _context.borrow_mut();
        let gp_regs = &mut context.gp_regs;
        gp_regs.rax = low as u64;
        gp_regs.rdx = high as u64;
        gp_regs.rip += 2;
    });
    Ok(())
}
