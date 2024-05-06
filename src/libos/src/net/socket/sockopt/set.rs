use crate::{fs::IoctlCmd, prelude::*};
use libc::ocall::setsockopt as do_setsockopt;

#[derive(Debug)]
pub struct SetSockOptRawCmd {
    level: i32,
    optname: i32,
    optval: &'static [u8],
}

impl SetSockOptRawCmd {
    pub fn new(level: i32, optname: i32, optval: &'static [u8]) -> Self {
        Self {
            level,
            optname,
            optval,
        }
    }

    pub fn execute(&mut self, fd: FileDesc) -> Result<()> {
        setsockopt_by_host(fd, self.level, self.optname, &self.optval)?;
        Ok(())
    }
}

impl IoctlCmd for SetSockOptRawCmd {}

pub fn setsockopt_by_host(fd: FileDesc, level: i32, optname: i32, optval: &[u8]) -> Result<()> {
    try_libc!(do_setsockopt(
        fd as _,
        level as _,
        optname as _,
        optval.as_ptr() as _,
        optval.len() as _
    ));
    Ok(())
}
