use io_uring_callback::IoUring;
use crate::prelude::*;

use crate::socket::{socket::Ipv4SocketAddr, runtime::Runtime, stream::StreamSocket};

use self::socket_file::SocketProtocol;

use super::AddressFamily;

pub type Ipv4Stream = StreamSocket<Ipv4SocketAddr, SocketRuntime>;

mod recv;
mod send;
mod socket_file;

pub struct SocketRuntime;

impl Runtime for SocketRuntime {
    fn io_uring() -> &'static IoUring {
        &*crate::io_uring::SINGLETON
    }
}

#[derive(Debug)]
pub struct Ipv4StreamSocket {
    socket: Ipv4Stream, 
}

impl Ipv4StreamSocket {
    pub fn new(
        domain: Domain,
        protocol: c_int,
        socket_type: Type,
        nonblocking: bool,
    ) -> Result<Self> {
        if domain != Domain::Netlink {
            let protocol = SocketProtocol::try_from(protocol)
                .map_err(|_| errno!(EINVAL, "Invalid or unsupported network protocol"))?;
            if protocol != SocketProtocol::IPPROTO_IP
                && protocol != SocketProtocol::IPPROTO_TCP
            {
                return_errno!(EPROTONOSUPPORT, "Protocol not supported");
            }
        }
        let any_socket = match domain {
            Domain::Ipv4 => {
                let ipv4_stream = Ipv4Stream::new(nonblocking)?;
                AnySocket::Ipv4Stream(ipv4_stream)
            }
            Domain::Ipv6 => {
                panic!()
            }
            Domain::Unix => {
                panic!()
            }
            Domain::Netlink => {
                return_errno!(ESOCKTNOSUPPORT, "netlink is a datagram-oriented service");
            }
        };
        let new_self = Self { socket: any_socket };
        Ok(new_self)
    }
    // pub fn new(
    //     domain: AddressFamily,
    //     socket_type: SocketType,
    //     file_flags: FileFlags,
    //     protocol: i32,
    // ) -> Result<Self> {
    //     let raw_host_fd = try_libc!(libc::ocall::socket(
    //         domain as i32,
    //         socket_type as i32 | file_flags.bits(),
    //         protocol
    //     )) as FileDesc;
    //     let host_fd = HostFd::new(raw_host_fd);
    //     Ok(HostSocket::from_host_fd(host_fd))
    // }

    // fn from_host_fd(host_fd: HostFd) -> HostSocket {
    //     let host_events = Atomic::new(IoEvents::empty());
    //     let notifier = IoNotifier::new();
    //     Self {
    //         host_fd,
    //         host_events,
    //         notifier,
    //     }
    // }

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

    // fn as_any(&self) -> &dyn core::any::Any {
    //     todo!()
    // }
}

pub trait Ipv4StreamSocketType {
    fn as_host_socket(&self) -> Result<&Ipv4StreamSocket>;
}

impl Ipv4StreamSocketType for FileRef {
    fn as_host_socket(&self) -> Result<&Ipv4StreamSocket> {
        self.as_any()
            .downcast_ref::<Ipv4StreamSocket>()
            .ok_or_else(|| errno!(EBADF, "not a host socket"))
    }
}
