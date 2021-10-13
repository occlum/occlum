use crate::prelude::*;

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use libc::ocall::ioctl_arg1 as do_ioctl;
    } else {
        use libc::ioctl as do_ioctl;
    }
}

const IFNAMSIZ: usize = 16;

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
    pub fn new(raw_cmd: u32) -> Self {
        Self {
            inner: GetIfReq::new(()),
            raw_cmd,
        }
    }

    pub fn output(&self) -> Option<&IfReq> {
        self.inner.output()
    }

    pub fn execute(&mut self, fd: HostFd) -> Result<()> {
        let if_req = get_ifreq_by_host(fd, self.raw_cmd)?;
        self.inner.set_output(if_req);
        Ok(())
    }
}

fn get_ifreq_by_host(fd: HostFd, cmd: u32) -> Result<IfReq> {
    let mut if_req: IfReq = Default::default();
    try_libc!(do_ioctl(
        fd as _,
        cmd as _,
        &mut if_req as *mut IfReq as *mut i32
    ));
    Ok(if_req)
}

impl IoctlCmd for GetIfReqWithRawCmd {}

async_io::impl_ioctl_cmd! {
    pub struct GetIfReq<Input=(), Output=IfReq> {}
}
