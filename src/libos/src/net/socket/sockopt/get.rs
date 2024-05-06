use crate::{fs::IoctlCmd, prelude::*};
use libc::ocall::getsockopt as do_getsockopt;

#[derive(Debug)]
pub struct GetSockOptRawCmd {
    level: i32,
    optname: i32,
    optval: Box<[u8]>,
    optlen: Option<u32>,
}

impl GetSockOptRawCmd {
    pub fn new(level: i32, optname: i32, max_optlen: u32) -> Self {
        // Using uninit slice is safe, since the buffer in rust SDK ocall is [out] type.
        let optval = unsafe { Box::new_uninit_slice(max_optlen as usize).assume_init() };
        Self {
            level,
            optname,
            optval,
            optlen: None,
        }
    }

    pub fn execute(&mut self, fd: FileDesc) -> Result<()> {
        if self.optlen.is_some() {
            return_errno!(EINVAL, "can not execute twice");
        }
        self.optlen = Some(getsockopt_by_host(
            fd,
            self.level,
            self.optname,
            &mut self.optval,
        )?);
        Ok(())
    }

    pub fn output(&self) -> Option<&[u8]> {
        self.optlen
            .map(|opt_len| &self.optval[..(opt_len as usize)])
    }
}

impl IoctlCmd for GetSockOptRawCmd {}

pub fn getsockopt_by_host(
    fd: FileDesc,
    level: i32,
    optname: i32,
    optval: &mut [u8],
) -> Result<u32> {
    let max_optlen = optval.len() as u32;
    let mut optlen = max_optlen;
    try_libc!(do_getsockopt(
        fd as _,
        level as _,
        optname as _,
        optval.as_mut_ptr() as _,
        &mut optlen as *mut u32
    ));
    // Defence Iago attack
    if optlen > max_optlen {
        return_errno!(EINVAL, "host returns a invalid optlen");
    }
    Ok(optlen)
}
