use super::set::setsockopt_by_host;
use crate::{fs::IoctlCmd, prelude::*};

crate::impl_ioctl_cmd! {
    pub struct SetSendBufSizeCmd<Input=usize, Output=()> {}
}

crate::impl_ioctl_cmd! {
    pub struct SetRecvBufSizeCmd<Input=usize, Output=()> {}
}

impl SetSendBufSizeCmd {
    pub fn update_host(&self, fd: FileDesc) -> Result<()> {
        // The buf size for host call should be divided by 2 because the value will be doubled by host kernel.
        let host_call_buf_size = (self.input / 2).to_ne_bytes();

        // Setting SO_SNDBUF for host socket needs to respect /proc/sys/net/core/wmem_max. Thus, the value might be different on host, but it is fine.
        setsockopt_by_host(
            fd,
            libc::SOL_SOCKET,
            super::SockOptName::SO_SNDBUF.into(),
            &host_call_buf_size,
        )
    }
}

impl SetRecvBufSizeCmd {
    pub fn update_host(&self, fd: FileDesc) -> Result<()> {
        // The buf size for host call should be divided by 2 because the value will be doubled by host kernel.
        let host_call_buf_size = (self.input / 2).to_ne_bytes();

        // Setting SO_RCVBUF for host socket needs to respect /proc/sys/net/core/rmem_max. Thus, the value might be different on host, but it is fine.
        setsockopt_by_host(
            fd,
            libc::SOL_SOCKET,
            super::SockOptName::SO_RCVBUF.into(),
            &host_call_buf_size,
        )
    }
}
