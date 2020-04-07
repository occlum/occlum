use super::table;
/// Process scheduling.
use crate::prelude::*;

pub fn do_sched_getaffinity(tid: pid_t, cpu_set: &mut CpuSet) -> Result<usize> {
    let host_tid = match tid {
        0 => 0,
        _ => find_host_tid(tid)?,
    };
    let buf = cpu_set.as_mut_ptr();
    let cpusize = cpu_set.len();
    let retval = try_libc!({
        let mut retval = 0;
        let sgx_status = occlum_ocall_sched_getaffinity(&mut retval, host_tid as i32, cpusize, buf);
        assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
        retval
    }) as usize;
    // Note: the first retval bytes in CpuSet are valid
    Ok(retval)
}

pub fn do_sched_setaffinity(tid: pid_t, cpu_set: &CpuSet) -> Result<()> {
    let host_tid = match tid {
        0 => 0,
        _ => find_host_tid(tid)?,
    };
    let buf = cpu_set.as_ptr();
    let cpusize = cpu_set.len();
    try_libc!({
        let mut retval = 0;
        let sgx_status = occlum_ocall_sched_setaffinity(&mut retval, host_tid as i32, cpusize, buf);
        assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
        retval
    });
    Ok(())
}

pub fn do_sched_yield() {
    unsafe {
        let status = occlum_ocall_sched_yield();
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
}

fn find_host_tid(tid: pid_t) -> Result<pid_t> {
    let thread = table::get_thread(tid)?;
    // TODO: fix the race condition of host_tid being available.
    let host_tid = thread
        .inner()
        .host_tid()
        .ok_or_else(|| errno!(ESRCH, "host_tid is not available"))?;
    Ok(host_tid)
}

pub struct CpuSet {
    vec: Vec<u8>,
}

impl CpuSet {
    pub fn new(len: usize) -> CpuSet {
        let mut cpuset = CpuSet {
            vec: Vec::with_capacity(len),
        };
        cpuset.vec.resize(len, 0);
        cpuset
    }

    pub fn from_raw_buf(ptr: *const u8, cpusize: usize) -> CpuSet {
        let mut cpuset = CpuSet {
            vec: Vec::with_capacity(cpusize),
        };
        let buf_slice = unsafe { std::slice::from_raw_parts(ptr, cpusize) };
        cpuset.vec.extend_from_slice(buf_slice);
        cpuset
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.vec.as_mut_ptr()
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.vec.as_ptr()
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.vec.as_mut_slice()
    }

    pub fn as_slice(&self) -> &[u8] {
        self.vec.as_slice()
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }
}

impl std::fmt::LowerHex for CpuSet {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in &(self.vec) {
            fmtr.write_fmt(format_args!("{:02x}", byte))?;
        }
        Ok(())
    }
}

impl std::fmt::UpperHex for CpuSet {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in &(self.vec) {
            fmtr.write_fmt(format_args!("{:02X}", byte))?;
        }
        Ok(())
    }
}

extern "C" {
    fn occlum_ocall_sched_getaffinity(
        ret: *mut i32,
        host_tid: i32,
        cpusetsize: size_t,
        mask: *mut c_uchar,
    ) -> sgx_status_t;
    fn occlum_ocall_sched_setaffinity(
        ret: *mut i32,
        host_tid: i32,
        cpusetsize: size_t,
        mask: *const c_uchar,
    ) -> sgx_status_t;
    fn occlum_ocall_sched_yield() -> sgx_status_t;
}
