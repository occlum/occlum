use super::cpu_set::{CpuSet, AVAIL_CPUSET};
use super::priority::{NiceValue, PrioWhich};
use crate::prelude::*;
use crate::util::mem_util::from_user::*;
use core::convert::TryFrom;

pub fn do_sched_yield() -> Result<isize> {
    super::do_sched_yield::do_sched_yield();
    Ok(0)
}

pub fn do_sched_getaffinity(pid: pid_t, buf_size: size_t, buf_ptr: *mut u8) -> Result<isize> {
    // Construct safe Rust types
    let buf_size = {
        if buf_size < CpuSet::len() {
            return_errno!(EINVAL, "buf size is not big enough");
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
        if buf_size < CpuSet::len() {
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

pub fn do_getcpu(cpu_ptr: *mut u32, node_ptr: *mut u32) -> Result<isize> {
    // Do pointers check
    match (cpu_ptr.is_null(), node_ptr.is_null()) {
        (true, true) => return Ok(0),
        (false, true) => check_mut_ptr(cpu_ptr)?,
        (true, false) => check_mut_ptr(node_ptr)?,
        (false, false) => {
            check_mut_ptr(cpu_ptr)?;
            check_mut_ptr(node_ptr)?;
        }
    }
    // Call the memory-safe do_getcpu
    let (cpu, node) = super::do_getcpu::do_getcpu()?;
    // Copy to user
    if !cpu_ptr.is_null() {
        unsafe {
            cpu_ptr.write(cpu);
        }
    }
    if !node_ptr.is_null() {
        unsafe {
            node_ptr.write(node);
        }
    }
    Ok(0)
}

pub fn do_set_priority(which: i32, who: i32, prio: i32) -> Result<isize> {
    let which = PrioWhich::try_from(which)?;
    let nice = NiceValue::from(prio);
    super::do_priority::do_set_priority(which, who, nice)?;
    Ok(0)
}

pub fn do_get_priority(which: i32, who: i32) -> Result<isize> {
    let which = PrioWhich::try_from(which)?;
    let nice = super::do_priority::do_get_priority(which, who)?;
    // To avoid negative return values, "getpriority()" will
    // not return the normal nice-value, but a negated value that
    // has been offset by 20 (ie it returns 40..1 instead of -20..19)
    Ok((20 - nice.to_raw_val()) as _)
}
