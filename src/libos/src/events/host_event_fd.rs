use std::time::Duration;

use crate::prelude::*;
use crate::time::{timespec_t, TIMERSLACK};

pub struct HostEventFd {
    host_fd: FileDesc,
}

impl HostEventFd {
    pub fn new() -> Result<Self> {
        const EFD_NONBLOCK: i32 = 1 << 11;
        let host_fd = try_libc!({
            let mut ret: i32 = 0;
            let status = occlum_ocall_eventfd(&mut ret, 0, EFD_NONBLOCK);
            assert!(status == sgx_status_t::SGX_SUCCESS);
            ret
        }) as FileDesc;
        Ok(Self { host_fd })
    }

    pub fn read_u64(&self) -> Result<u64> {
        let mut val: u64 = 0;
        let ret = try_libc!(libc::ocall::read(
            self.host_fd as c_int,
            &mut val as *mut _ as *mut c_void,
            std::mem::size_of::<u64>(),
        )) as usize;
        debug_assert!(ret != std::mem::size_of::<u64>());
        Ok(val)
    }

    pub fn write_u64(&self, val: u64) {
        unsafe {
            libc::ocall::write(
                self.host_fd as c_int,
                &val as *const _ as *const c_void,
                std::mem::size_of::<u64>(),
            );
        }
    }

    pub fn poll(&self, timeout: Option<&Duration>) -> Result<()> {
        let mut timeout = timeout.cloned();
        self.poll_mut(timeout.as_mut())
    }

    pub fn poll_mut(&self, timeout: Option<&mut Duration>) -> Result<()> {
        match timeout {
            None => ocall_eventfd_poll(self.host_fd, std::ptr::null_mut()),
            Some(timeout) => {
                const ZERO: Duration = Duration::from_secs(0);
                if *timeout == ZERO {
                    return_errno!(ETIMEDOUT, "should return immediately");
                }

                let mut remain_c = timespec_t::from(*timeout);
                let ret = ocall_eventfd_poll(self.host_fd, &mut remain_c);

                let remain = remain_c.as_duration();
                assert!(remain <= *timeout + TIMERSLACK.to_duration());
                *timeout = remain;

                // Poll syscall does not treat timeout as error. So we need
                // to distinguish the case by ourselves.
                if *timeout == ZERO {
                    return_errno!(ETIMEDOUT, "time is up");
                }

                ret
            }
        }
    }

    /// Write to all host eventfds in one OCall.
    ///
    /// Precondition. The caller must ensure that the host fds are valid.
    pub unsafe fn write_u64_raw_and_batch(host_fds: &[FileDesc], val: u64) {
        if host_fds.len() == 0 {
            return;
        }

        ocall_eventfd_write_batch(host_fds, val);
    }

    pub fn host_fd(&self) -> FileDesc {
        self.host_fd
    }
}

impl Drop for HostEventFd {
    fn drop(&mut self) {
        let ret = unsafe { libc::ocall::close(self.host_fd as c_int) };
        debug_assert!(ret == 0);
    }
}

fn ocall_eventfd_poll(host_fd: FileDesc, timeout: *mut timespec_t) -> Result<()> {
    try_libc!({
        let mut ret = 0;
        let status = unsafe { occlum_ocall_eventfd_poll(&mut ret, host_fd, timeout) };
        assert!(status == sgx_status_t::SGX_SUCCESS);
        ret
    });
    Ok(())
}

fn ocall_eventfd_write_batch(host_fds: &[FileDesc], val: u64) {
    let status =
        unsafe { occlum_ocall_eventfd_write_batch(host_fds.as_ptr(), host_fds.len(), val) };
    assert!(status == sgx_status_t::SGX_SUCCESS);
}

extern "C" {
    fn occlum_ocall_eventfd(ret: *mut i32, init_val: u32, flags: i32) -> sgx_status_t;
    fn occlum_ocall_eventfd_poll(
        ret: *mut i32,
        fd: FileDesc,
        timeout: *mut timespec_t,
    ) -> sgx_status_t;
    fn occlum_ocall_eventfd_write_batch(
        fds: *const FileDesc,
        num_fds: usize,
        val: u64,
    ) -> sgx_status_t;
}
