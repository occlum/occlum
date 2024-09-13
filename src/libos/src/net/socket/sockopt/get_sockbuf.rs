use core::{mem, slice};

use super::getsockopt_by_host;
use crate::prelude::*;

crate::impl_ioctl_cmd! {
    pub struct GetSendBufSizeCmd<Input=(), Output=usize> {}
}

crate::impl_ioctl_cmd! {
    pub struct GetRecvBufSizeCmd<Input=(), Output=usize> {}
}

impl GetSendBufSizeCmd {
    pub fn execute(&mut self, fd: FileDesc) -> Result<()> {
        let mut buf_size = 0_i32;
        let buf_ref = unsafe {
            slice::from_raw_parts_mut(&buf_size as *const _ as *mut u8, mem::size_of::<i32>())
        };

        getsockopt_by_host(
            fd,
            libc::SOL_SOCKET,
            super::SockOptName::SO_SNDBUF.into(),
            buf_ref,
        )?;

        self.set_output(buf_size as usize);
        Ok(())
    }
}

impl GetRecvBufSizeCmd {
    pub fn execute(&mut self, fd: FileDesc) -> Result<()> {
        let mut buf_size = 0_i32;
        let buf_ref = unsafe {
            slice::from_raw_parts_mut(&buf_size as *const _ as *mut u8, mem::size_of::<i32>())
        };

        getsockopt_by_host(
            fd,
            libc::SOL_SOCKET,
            super::SockOptName::SO_RCVBUF.into(),
            buf_ref,
        )?;

        self.set_output(buf_size as usize);
        Ok(())
    }
}
