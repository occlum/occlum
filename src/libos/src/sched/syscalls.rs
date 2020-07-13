use super::cpu_set::{CpuSet, AVAIL_CPUSET};
use crate::prelude::*;
use crate::util::mem_util::from_user::*;

pub fn do_sched_yield() -> Result<isize> {
    super::do_sched_yield::do_sched_yield();
    Ok(0)
}

pub fn do_sched_getaffinity(pid: pid_t, buf_size: size_t, buf_ptr: *mut u8) -> Result<isize> {
    // Construct safe Rust types
    let buf_size = {
        if buf_size * 8 < AVAIL_CPUSET.cpu_count() {
            return_errno!(EINVAL, "buf size is not big enough");
        }

        // Linux stores the cpumask in an array of "unsigned long" so the buffer needs to be
        // multiple of unsigned long. However, Occlum doesn't have this restriction.
        if (buf_size & (std::mem::size_of::<u64>() - 1) != 0) {
            warn!("cpuset buf size is not a multiple of unsigned long");
        }
        CpuSet::len()
    };
    let mut buf_slice = {
        check_mut_array(buf_ptr, buf_size)?;
        if buf_ptr as *const _ == std::ptr::null() {
            return_errno!(EFAULT, "buf ptr must NOT be null");
        }
        unsafe { std::slice::from_raw_parts_mut(buf_ptr, buf_size) }
    };
    // Call the memory-safe do_sched_getaffinity
    let affinity = super::do_sched_affinity::do_sched_getaffinity(pid)?;
    debug_assert!(affinity.as_slice().len() == CpuSet::len());
    // Copy from Rust types to C types
    buf_slice.copy_from_slice(affinity.as_slice());
    Ok(CpuSet::len() as isize)
}

pub fn do_sched_setaffinity(pid: pid_t, buf_size: size_t, buf_ptr: *const u8) -> Result<isize> {
    // Convert unsafe C types into safe Rust types
    let buf_size = {
        if buf_size * 8 < AVAIL_CPUSET.cpu_count() {
            return_errno!(EINVAL, "buf size is not big enough");
        }
        CpuSet::len()
    };
    let buf_slice = {
        check_array(buf_ptr, buf_size)?;
        if buf_ptr as *const _ == std::ptr::null() {
            return_errno!(EFAULT, "buf ptr must NOT be null");
        }
        unsafe { std::slice::from_raw_parts(buf_ptr, buf_size) }
    };
    // Call the memory-safe do_sched_setaffinity
    let affinity = CpuSet::from_slice(buf_slice).unwrap();
    super::do_sched_affinity::do_sched_setaffinity(pid, affinity)?;
    Ok(0)
}
