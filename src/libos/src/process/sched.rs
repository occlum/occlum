use super::*;

extern "C" {
    fn occlum_ocall_sched_getaffinity(
        ret: *mut i32,
        errno: *mut i32,
        pid: i32,
        cpusetsize: size_t,
        mask: *mut c_uchar,
    ) -> sgx_status_t;
    fn occlum_ocall_sched_setaffinity(
        ret: *mut i32,
        errno: *mut i32,
        pid: i32,
        cpusetsize: size_t,
        mask: *const c_uchar,
    ) -> sgx_status_t;
    fn occlum_ocall_sched_yield() -> sgx_status_t;
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

fn find_host_tid(pid: pid_t) -> Result<pid_t> {
    let process_ref = if pid == 0 { get_current() } else { get(pid)? };
    let mut process = process_ref.lock().unwrap();
    let host_tid = process.get_host_tid();
    Ok(host_tid)
}

pub fn do_sched_getaffinity(pid: pid_t, cpu_set: &mut CpuSet) -> Result<i32> {
    let host_tid = match pid {
        0 => 0,
        _ => find_host_tid(pid)?,
    };
    let buf = cpu_set.as_mut_ptr();
    let cpusize = cpu_set.len();
    let mut ret = 0;
    let mut error = 0;
    unsafe {
        occlum_ocall_sched_getaffinity(&mut ret, &mut error, host_tid as i32, cpusize, buf);
    }
    if (ret < 0) {
        let errno = Errno::from(error as u32);
        return_errno!(errno, "occlum_ocall_sched_getaffinity failed");
    }
    Ok(ret)
}

pub fn do_sched_setaffinity(pid: pid_t, cpu_set: &CpuSet) -> Result<i32> {
    let host_tid = match pid {
        0 => 0,
        _ => find_host_tid(pid)?,
    };
    let buf = cpu_set.as_ptr();
    let cpusize = cpu_set.len();
    let mut ret = 0;
    let mut error = 0;
    unsafe {
        occlum_ocall_sched_setaffinity(&mut ret, &mut error, host_tid as i32, cpusize, buf);
    }
    if (ret < 0) {
        let errno = Errno::from(error as u32);
        return_errno!(errno, "occlum_ocall_sched_setaffinity failed");
    }
    Ok(ret)
}

pub fn do_sched_yield() {
    unsafe {
        let status = occlum_ocall_sched_yield();
        assert!(status == sgx_status_t::SGX_SUCCESS);
    }
}
