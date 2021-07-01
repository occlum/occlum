use std::any::Any;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem;

use atomic::Atomic;

use super::*;
use crate::fs::{
    occlum_ocall_ioctl, AccessMode, CreationFlags, File, FileRef, HostFd, IoEvents, IoNotifier,
    IoctlCmd, StatusFlags,
};

mod ioctl_impl;
mod recv;
mod send;
mod socket_file;

/// Native linux socket
#[derive(Debug)]
pub struct HostSocket {
    host_fd: HostFd,
    host_events: Atomic<IoEvents>,
    notifier: IoNotifier,
}

impl HostSocket {
    pub fn new(
        domain: AddressFamily,
        socket_type: SocketType,
        file_flags: FileFlags,
        protocol: i32,
    ) -> Result<Self> {
        let raw_host_fd = try_libc!(libc::ocall::socket(
            domain as i32,
            socket_type as i32 | file_flags.bits(),
            protocol
        )) as FileDesc;
        let host_fd = HostFd::new(raw_host_fd);
        Ok(HostSocket::from_host_fd(host_fd))
    }

    fn from_host_fd(host_fd: HostFd) -> HostSocket {
        let host_events = Atomic::new(IoEvents::empty());
        let notifier = IoNotifier::new();
        Self {
            host_fd,
            host_events,
            notifier,
        }
    }

    pub fn bind(&self, addr: &SockAddr) -> Result<()> {
        let (addr_ptr, addr_len) = addr.as_ptr_and_len();

        let ret = try_libc!(libc::ocall::bind(
            self.raw_host_fd() as i32,
            addr_ptr as *const libc::sockaddr,
            addr_len as u32
        ));
        Ok(())
    }

    pub fn listen(&self, backlog: i32) -> Result<()> {
        let ret = try_libc!(libc::ocall::listen(self.raw_host_fd() as i32, backlog));
        Ok(())
    }

    pub fn accept(&self, flags: FileFlags) -> Result<(Self, Option<SockAddr>)> {
        let mut sockaddr = SockAddr::default();
        let mut addr_len = sockaddr.len();

        let raw_host_fd = try_libc!(libc::ocall::accept4(
            self.raw_host_fd() as i32,
            sockaddr.as_mut_ptr() as *mut _,
            &mut addr_len as *mut _ as *mut _,
            flags.bits()
        )) as FileDesc;
        let host_fd = HostFd::new(raw_host_fd);

        let addr_option = if addr_len != 0 {
            sockaddr.set_len(addr_len)?;
            Some(sockaddr)
        } else {
            None
        };
        Ok((HostSocket::from_host_fd(host_fd), addr_option))
    }

    pub fn connect(&self, addr: &Option<SockAddr>) -> Result<()> {
        debug!("connect: host_fd: {}, addr {:?}", self.raw_host_fd(), addr);

        let (addr_ptr, addr_len) = if let Some(sock_addr) = addr {
            sock_addr.as_ptr_and_len()
        } else {
            (std::ptr::null(), 0)
        };

        let ret = try_libc!(libc::ocall::connect(
            self.raw_host_fd() as i32,
            addr_ptr,
            addr_len as u32
        ));
        Ok(())
    }

    pub fn sendto(
        &self,
        buf: &[u8],
        flags: SendFlags,
        addr_option: &Option<SockAddr>,
    ) -> Result<usize> {
        let bufs = vec![buf];
        let name_option = addr_option.as_ref().map(|addr| addr.as_slice());
        self.do_sendmsg(&bufs, flags, name_option, None)
    }

    pub fn recvfrom(&self, buf: &mut [u8], flags: RecvFlags) -> Result<(usize, Option<SockAddr>)> {
        let mut sockaddr = SockAddr::default();
        let mut bufs = vec![buf];
        let (bytes_recv, addr_len, _, _) =
            self.do_recvmsg(&mut bufs, flags, Some(sockaddr.as_mut_slice()), None)?;

        let addr_option = if addr_len != 0 {
            sockaddr.set_len(addr_len)?;
            Some(sockaddr)
        } else {
            None
        };
        Ok((bytes_recv, addr_option))
    }

    pub fn raw_host_fd(&self) -> FileDesc {
        self.host_fd.to_raw()
    }

    pub fn shutdown(&self, how: HowToShut) -> Result<()> {
        try_libc!(libc::ocall::shutdown(self.raw_host_fd() as i32, how.bits()));
        Ok(())
    }
}

pub trait HostSocketType {
    fn as_host_socket(&self) -> Result<&HostSocket>;
}

impl HostSocketType for FileRef {
    fn as_host_socket(&self) -> Result<&HostSocket> {
        self.as_any()
            .downcast_ref::<HostSocket>()
            .ok_or_else(|| errno!(EBADF, "not a host socket"))
    }
}
