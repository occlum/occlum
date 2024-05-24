use crate::{fs::IoctlCmd, prelude::*};
use libc::ocall::setsockopt as do_setsockopt;

#[derive(Debug)]
pub struct SetSockOptRawCmd<'a> {
    level: i32,
    optname: i32,
    optval: &'a [u8],
}

impl IoctlCmd for SetSockOptRawCmd<'static> {}

impl<'a> SetSockOptRawCmd<'a> {
    pub fn new(level: i32, optname: i32, optval: &'a [u8]) -> SetSockOptRawCmd<'a> {
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
