use crate::prelude::*;
use crate::syscall::CpuContext;

pub const RDTSC_OPCODE: u16 = 0x310F;

pub fn handle_rdtsc_exception(user_context: &mut CpuContext) -> Result<isize> {
    debug!("handle RDTSC exception");
    let (low, high) = crate::time::do_rdtsc();
    trace!("do_rdtsc result {{ low: {:#x} high: {:#x}}}", low, high);
    user_context.rax = low as u64;
    user_context.rdx = high as u64;
    user_context.rip += 2;

    Ok(0)
}
