use super::*;
use crate::process::table;
use crate::time::up_time::get;
use crate::vm::USER_SPACE_VM_MANAGER;
use config::LIBOS_CONFIG;

// This structure aligns with Linux kernels which are later than v2.3.23 (i386) and v2.3.48 (all architectures)
#[repr(C)]
#[derive(Clone, Default)]
pub struct sysinfo_t {
    uptime: i64,
    loads: [u64; 3],
    totalram: u64,
    freeram: u64,
    sharedram: u64,
    bufferram: u64,
    totalswap: u64,
    freeswap: u64,
    procs: u16,
    totalhigh: u64,
    freehigh: u64,
    mem_unit: u32,
}

pub async fn do_sysinfo() -> Result<sysinfo_t> {
    let info = sysinfo_t {
        uptime: time::up_time::get().unwrap().as_secs() as i64, // Duration can't be negative
        totalram: USER_SPACE_VM_MANAGER.get_total_size() as u64,
        freeram: current!().vm().get_free_size().await as u64,
        procs: table::get_all_processes().len() as u16,
        mem_unit: 1,
        ..Default::default()
    };
    Ok(info)
}
