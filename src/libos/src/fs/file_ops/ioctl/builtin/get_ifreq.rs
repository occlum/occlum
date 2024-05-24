use crate::prelude::*;

const IFNAMSIZ: usize = 16;
use super::IoctlCmd;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct IfReq {
    pub ifr_name: [u8; IFNAMSIZ],
    pub ifr_union: [u8; 24],
}

/// Many of the socket ioctl commands use `IfReq` as the structure to get configuration
/// of network devices. The only difference is the command number.
///
/// This structure wraps the `GetIfReq` and the command number as the `IoctlCmd`.
#[derive(Debug)]
pub struct GetIfReqWithRawCmd {
    inner: GetIfReq,
    raw_cmd: u32,
}

impl GetIfReqWithRawCmd {
    pub fn new(raw_cmd: u32, req: IfReq) -> Self {
        Self {
            inner: GetIfReq::new(req),
            raw_cmd,
        }
    }

    pub fn output(&self) -> Option<&IfReq> {
        self.inner.output()
    }

    pub fn execute(&mut self, fd: FileDesc) -> Result<()> {
        let input_if_req = self.inner.input();
        let output_if_req = GetIfReqWithRawCmd::get_ifreq_by_host(fd, self.raw_cmd, input_if_req)?;
        self.inner.set_output(output_if_req);
        Ok(())
    }

    fn get_ifreq_by_host(fd: FileDesc, cmd: u32, req: &IfReq) -> Result<IfReq> {
        let mut if_req: IfReq = req.clone();
        try_libc!({
            let mut retval: i32 = 0;
            extern "C" {
                pub fn occlum_ocall_ioctl(
                    ret: *mut i32,
                    fd: c_int,
                    request: c_int,
                    arg: *mut c_void,
                    len: size_t,
                ) -> sgx_types::sgx_status_t;
            }

            use libc::{c_int, c_void, size_t};
            use occlum_ocall_ioctl as do_ioctl;

            let status = do_ioctl(
                &mut retval as *mut i32,
                fd as i32,
                cmd as i32,
                &mut if_req as *mut IfReq as *mut c_void,
                std::mem::size_of::<IfReq>(),
            );
            assert!(status == sgx_types::sgx_status_t::SGX_SUCCESS);
            retval
        });
        Ok(if_req)
    }
}

impl IoctlCmd for GetIfReqWithRawCmd {}

impl_ioctl_cmd! {
    pub struct GetIfReq<Input=IfReq, Output=IfReq> {}
}
