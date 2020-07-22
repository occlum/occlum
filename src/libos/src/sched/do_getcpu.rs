use super::cpu_set::{CpuSet, NCORES};
use crate::prelude::*;
use crate::process::ThreadRef;

pub fn do_getcpu() -> Result<(u32, u32)> {
    let cpu = pick_cpu_within_affinity_mask();
    let node = NUMA_TOPOLOGY[cpu as usize];
    debug!("do_getcpu cpu = {}, node = {}", cpu, node);
    Ok((cpu, node))
}

fn pick_cpu_within_affinity_mask() -> u32 {
    // Always return the idx of the first bit in the affnity mask for now.
    // TODO: randomly choose a bit in the affinity mask.
    let thread = current!();
    let sched = thread.sched().lock().unwrap();
    let idx = sched.affinity().first_cpu_idx().unwrap();
    idx as u32
}

fn validate_numa_topology(numa_topology: &Vec<u32>) -> Result<()> {
    for node_id in numa_topology.iter() {
        if *node_id >= numa_topology.len() as u32 {
            return_errno!(EINVAL, "NUMA node id exceeds the core numbers");
        }
    }
    Ok(())
}

lazy_static! {
    /// The information of Non-Uniform Memory Access(NUMA) topology
    pub static ref NUMA_TOPOLOGY: Vec<u32> = {
        extern "C" {
            fn occlum_ocall_get_numa_topology(ret: *mut i32, numa_buf: *mut u32, ncpus: usize) -> sgx_status_t;
        }
        let mut numa_topology = vec![0; *NCORES];
        let mut retval: i32 = 0;
        let status = unsafe { occlum_ocall_get_numa_topology(&mut retval, numa_topology.as_mut_ptr(), numa_topology.len()) };
        assert!(status == sgx_status_t::SGX_SUCCESS);
        validate_numa_topology(&numa_topology).expect("ocall returned invalid NUMA topology");
        numa_topology
    };
}
