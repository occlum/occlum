use super::*;
use sgx_types::*;

pub const RDTSC_OPCODE: u16 = 0x310F;

pub fn handle_rdtsc_exception(info: &mut sgx_exception_info_t) -> u32 {
    debug!("handle RDTSC exception");
    let (low, high) = time::do_rdtsc();
    trace!("do_rdtsc result {{ low: {:#x} high: {:#x}}}", low, high);
    info.cpu_context.rax = low as u64;
    info.cpu_context.rdx = high as u64;
    info.cpu_context.rip += 2;

    EXCEPTION_CONTINUE_EXECUTION
}
