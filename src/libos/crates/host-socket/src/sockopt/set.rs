cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use libc::ocall::setsockopt as do_setsockopt;
    } else {
        use libc::setsockopt as do_setsockopt;
    }
}

use crate::prelude::*;

#[derive(Debug)]
pub struct SetSockOptRawCmd {
    level: i32,
    optname: i32,
    optval: Box<[u8]>,
}

impl SetSockOptRawCmd {
    pub fn new(level: i32, optname: i32, optval: &[u8]) -> Self {
        let optval = Box::from(optval);
        Self {
            level,
            optname,
            optval,
        }
    }

    pub fn execute(&mut self, fd: HostFd) -> Result<()> {
        setsockopt_by_host(fd, self.level, self.optname, &self.optval)?;
        Ok(())
    }
}

impl IoctlCmd for SetSockOptRawCmd {}

fn setsockopt_by_host(fd: HostFd, level: i32, optname: i32, optval: &[u8]) -> Result<()> {
    try_libc!(do_setsockopt(
        fd as _,
        level as _,
        optname as _,
        optval.as_ptr() as _,
        optval.len() as _
    ));
    Ok(())
}
