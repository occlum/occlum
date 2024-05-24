use self::impls::{Ipv4Datagram, Ipv6Datagram};
use crate::events::{Observer, Poller};
use crate::net::socket::{MsgFlags, SocketProtocol};

use self::impls::{Ipv4Stream, Ipv6Stream};
use crate::fs::{AccessMode, IoEvents, IoNotifier, IoctlCmd, StatusFlags};
use crate::net::socket::{AnyAddr, Ipv4SocketAddr, Ipv6SocketAddr};
use crate::prelude::*;

#[derive(Debug)]
pub struct SocketFile {
    socket: AnySocket,
}

// Apply a function to all variants of AnySocket enum.
macro_rules! apply_fn_on_any_socket {
    ($any_socket:expr, |$socket:ident| { $($fn_body:tt)* }) => {{
        let any_socket: &AnySocket = $any_socket;
        match any_socket {
            AnySocket::Ipv4Stream($socket) => {
                $($fn_body)*
            }
            AnySocket::Ipv6Stream($socket) => {
                $($fn_body)*
            }
            AnySocket::Ipv4Datagram($socket) => {
                $($fn_body)*
            }
            AnySocket::Ipv6Datagram($socket) => {
                $($fn_body)*
            }
        }
    }}
}

pub trait UringSocketType {
    fn as_uring_socket(&self) -> Result<&SocketFile>;
}

impl UringSocketType for FileRef {
    fn as_uring_socket(&self) -> Result<&SocketFile> {
        self.as_any()
            .downcast_ref::<SocketFile>()
            .ok_or_else(|| errno!(ENOTSOCK, "not a uring socket"))
    }
}

#[derive(Debug)]
enum AnySocket {
    Ipv4Stream(Ipv4Stream),
    Ipv6Stream(Ipv6Stream),
    Ipv4Datagram(Ipv4Datagram),
    Ipv6Datagram(Ipv6Datagram),
}

// Implement the common methods required by FileHandle
impl SocketFile {
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.read(buf) })
    }

    pub fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.readv(bufs) })
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.write(buf) })
    }

    pub fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.writev(bufs) })
    }

    pub fn access_mode(&self) -> AccessMode {
        // We consider all sockets both readable and writable
        AccessMode::O_RDWR
    }

    pub fn status_flags(&self) -> StatusFlags {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.status_flags() })
    }

    pub fn host_fd_inner(&self) -> FileDesc {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.host_fd() })
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        apply_fn_on_any_socket!(&self.socket, |socket| {
            socket.set_status_flags(new_flags)
        })
    }

    pub fn notifier(&self) -> &IoNotifier {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.notifier() })
    }

    pub fn poll(&self, mask: IoEvents, poller: Option<&Poller>) -> IoEvents {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.poll(mask, poller) })
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.ioctl(cmd) })
    }

    pub fn get_type(&self) -> SocketType {
        match self.socket {
            AnySocket::Ipv4Stream(_) | AnySocket::Ipv6Stream(_) => SocketType::STREAM,
            AnySocket::Ipv4Datagram(_) | AnySocket::Ipv6Datagram(_) => SocketType::DGRAM,
        }
    }
}

// Implement socket-specific methods
impl SocketFile {
    pub fn new(
        domain: Domain,
        protocol: SocketProtocol,
        socket_type: SocketType,
        nonblocking: bool,
    ) -> Result<Self> {
        match socket_type {
            SocketType::STREAM => {
                if protocol != SocketProtocol::IPPROTO_IP && protocol != SocketProtocol::IPPROTO_TCP
                {
                    return_errno!(EPROTONOSUPPORT, "Protocol not supported");
                }
                let any_socket = match domain {
                    Domain::INET => {
                        let ipv4_stream = Ipv4Stream::new(nonblocking)?;
                        AnySocket::Ipv4Stream(ipv4_stream)
                    }
                    Domain::INET6 => {
                        let ipv6_stream = Ipv6Stream::new(nonblocking)?;
                        AnySocket::Ipv6Stream(ipv6_stream)
                    }
                    _ => {
                        panic!()
                    }
                };
                let new_self = Self { socket: any_socket };
                Ok(new_self)
            }
            SocketType::DGRAM => {
                if protocol != SocketProtocol::IPPROTO_IP && protocol != SocketProtocol::IPPROTO_UDP
                {
                    return_errno!(EPROTONOSUPPORT, "Protocol not supported");
                }
                let any_socket = match domain {
                    Domain::INET => {
                        let ipv4_datagram = Ipv4Datagram::new(nonblocking)?;
                        AnySocket::Ipv4Datagram(ipv4_datagram)
                    }
                    Domain::INET6 => {
                        let ipv6_datagram = Ipv6Datagram::new(nonblocking)?;
                        AnySocket::Ipv6Datagram(ipv6_datagram)
                    }
                    _ => {
                        return_errno!(EINVAL, "not support yet");
                    }
                };
                let new_self = Self { socket: any_socket };
                Ok(new_self)
            }
            SocketType::RAW => {
                return_errno!(EINVAL, "RAW socket not supported");
            }
            _ => {
                return_errno!(ESOCKTNOSUPPORT, "socket type not supported");
            }
        }
    }

    pub fn domain(&self) -> Domain {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.domain() })
    }

    pub fn is_stream(&self) -> bool {
        matches!(&self.socket, AnySocket::Ipv4Stream(_))
    }

    pub fn connect(&self, addr: &AnyAddr) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let ip_addr = addr.to_ipv4()?;
                ipv4_stream.connect(ip_addr)
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let ip_addr = addr.to_ipv6()?;
                ipv6_stream.connect(ip_addr)
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let mut ip_addr = None;
                if !addr.is_unspec() {
                    let ipv4_addr = addr.to_ipv4()?;
                    ip_addr = Some(ipv4_addr);
                }
                ipv4_datagram.connect(ip_addr)
            }
            AnySocket::Ipv6Datagram(ipv6_datagram) => {
                let mut ip_addr = None;
                if !addr.is_unspec() {
                    let ipv6_addr = addr.to_ipv6()?;
                    ip_addr = Some(ipv6_addr);
                }
                ipv6_datagram.connect(ip_addr)
            }
            _ => {
                return_errno!(EINVAL, "connect is not supported");
            }
        }
    }

    pub fn bind(&self, addr: &AnyAddr) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let ip_addr = addr.to_ipv4()?;
                ipv4_stream.bind(ip_addr)
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let ip_addr = addr.to_ipv6()?;
                ipv6_stream.bind(ip_addr)
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let ip_addr = addr.to_ipv4()?;
                ipv4_datagram.bind(ip_addr)
            }

            AnySocket::Ipv6Datagram(ipv6_datagram) => {
                let ip_addr = addr.to_ipv6()?;
                ipv6_datagram.bind(ip_addr)
            }

            _ => {
                return_errno!(EINVAL, "bind is not supported");
            }
        }
    }

    pub fn listen(&self, backlog: u32) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ip_stream) => ip_stream.listen(backlog),
            AnySocket::Ipv6Stream(ip_stream) => ip_stream.listen(backlog),
            _ => {
                return_errno!(EOPNOTSUPP, "The socket is not of a listen supported type");
            }
        }
    }

    pub fn accept(&self, nonblocking: bool) -> Result<Self> {
        let accepted_any_socket = match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let accepted_ipv4_stream = ipv4_stream.accept(nonblocking)?;
                AnySocket::Ipv4Stream(accepted_ipv4_stream)
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let accepted_ipv6_stream = ipv6_stream.accept(nonblocking)?;
                AnySocket::Ipv6Stream(accepted_ipv6_stream)
            }
            _ => {
                return_errno!(EOPNOTSUPP, "The socket is not of a accept supported type");
            }
        };
        let accepted_socket_file = SocketFile {
            socket: accepted_any_socket,
        };
        Ok(accepted_socket_file)
    }

    pub fn recvfrom(&self, buf: &mut [u8], flags: RecvFlags) -> Result<(usize, Option<AnyAddr>)> {
        let (bytes_recv, addr_recv, _, _) = self.recvmsg(&mut [buf], flags, None)?;
        Ok((bytes_recv, addr_recv))
    }

    pub fn recvmsg(
        &self,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
        control: Option<&mut [u8]>,
    ) -> Result<(usize, Option<AnyAddr>, MsgFlags, usize)> {
        // TODO: support msg_flags and msg_control
        Ok(match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let (bytes_recv, addr_recv, msg_flags) = ipv4_stream.recvmsg(bufs, flags)?;
                (
                    bytes_recv,
                    addr_recv.map(|addr| AnyAddr::Ipv4(addr)),
                    msg_flags,
                    0,
                )
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let (bytes_recv, addr_recv, msg_flags) = ipv6_stream.recvmsg(bufs, flags)?;
                (
                    bytes_recv,
                    addr_recv.map(|addr| AnyAddr::Ipv6(addr)),
                    msg_flags,
                    0,
                )
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let (bytes_recv, addr_recv, msg_flags, msg_controllen) =
                    ipv4_datagram.recvmsg(bufs, flags, control)?;
                (
                    bytes_recv,
                    addr_recv.map(|addr| AnyAddr::Ipv4(addr)),
                    msg_flags,
                    msg_controllen,
                )
            }
            AnySocket::Ipv6Datagram(ipv6_datagram) => {
                let (bytes_recv, addr_recv, msg_flags, msg_controllen) =
                    ipv6_datagram.recvmsg(bufs, flags, control)?;
                (
                    bytes_recv,
                    addr_recv.map(|addr| AnyAddr::Ipv6(addr)),
                    msg_flags,
                    msg_controllen,
                )
            }
            _ => {
                return_errno!(EINVAL, "recvfrom is not supported");
            }
        })
    }

    pub fn sendto(&self, buf: &[u8], addr: Option<AnyAddr>, flags: SendFlags) -> Result<usize> {
        self.sendmsg(&[buf], addr, flags, None)
    }

    pub fn sendmsg(
        &self,
        bufs: &[&[u8]],
        addr: Option<AnyAddr>,
        flags: SendFlags,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        let res = match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => ipv4_stream.sendmsg(bufs, flags),
            AnySocket::Ipv6Stream(ipv6_stream) => ipv6_stream.sendmsg(bufs, flags),
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let ip_addr = if let Some(addr) = addr.as_ref() {
                    Some(addr.to_ipv4()?)
                } else {
                    None
                };
                ipv4_datagram.sendmsg(bufs, ip_addr, flags, control)
            }
            AnySocket::Ipv6Datagram(ipv6_datagram) => {
                let ip_addr = if let Some(addr) = addr.as_ref() {
                    Some(addr.to_ipv6()?)
                } else {
                    None
                };
                ipv6_datagram.sendmsg(bufs, ip_addr, flags, control)
            }
            _ => {
                return_errno!(EINVAL, "sendmsg is not supported");
            }
        };
        if res.has_errno(EPIPE) && !flags.contains(SendFlags::MSG_NOSIGNAL) {
            crate::signal::do_tkill(current!().tid(), crate::signal::SIGPIPE.as_u8() as i32);
        }

        res
    }

    pub fn addr(&self) -> Result<AnyAddr> {
        Ok(match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => AnyAddr::Ipv4(ipv4_stream.addr()?),
            AnySocket::Ipv6Stream(ipv6_stream) => AnyAddr::Ipv6(ipv6_stream.addr()?),
            AnySocket::Ipv4Datagram(ipv4_datagram) => AnyAddr::Ipv4(ipv4_datagram.addr()?),
            AnySocket::Ipv6Datagram(ipv6_datagram) => AnyAddr::Ipv6(ipv6_datagram.addr()?),
            _ => {
                return_errno!(EINVAL, "addr is not supported");
            }
        })
    }

    pub fn peer_addr(&self) -> Result<AnyAddr> {
        Ok(match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => AnyAddr::Ipv4(ipv4_stream.peer_addr()?),
            AnySocket::Ipv6Stream(ipv6_stream) => AnyAddr::Ipv6(ipv6_stream.peer_addr()?),
            AnySocket::Ipv4Datagram(ipv4_datagram) => AnyAddr::Ipv4(ipv4_datagram.peer_addr()?),
            AnySocket::Ipv6Datagram(ipv6_datagram) => AnyAddr::Ipv6(ipv6_datagram.peer_addr()?),
            _ => {
                return_errno!(EINVAL, "peer_addr is not supported");
            }
        })
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => ipv4_stream.shutdown(how),
            AnySocket::Ipv6Stream(ipv6_stream) => ipv6_stream.shutdown(how),
            AnySocket::Ipv4Datagram(ipv4_datagram) => ipv4_datagram.shutdown(how),
            AnySocket::Ipv6Datagram(ipv6_datagram) => ipv6_datagram.shutdown(how),
            _ => {
                return_errno!(EINVAL, "shutdown is not supported");
            }
        }
    }

    pub fn close(&self) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => ipv4_stream.close(),
            AnySocket::Ipv6Stream(ipv6_stream) => ipv6_stream.close(),
            AnySocket::Ipv4Datagram(ipv4_datagram) => ipv4_datagram.close(),
            AnySocket::Ipv6Datagram(ipv6_datagram) => ipv6_datagram.close(),
            _ => Ok(()),
        }
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        self.close();
    }
}

mod impls {
    use super::*;
    use io_uring_callback::IoUring;

    pub type Ipv4Stream =
        crate::net::socket::uring::stream::StreamSocket<Ipv4SocketAddr, SocketRuntime>;
    pub type Ipv6Stream =
        crate::net::socket::uring::stream::StreamSocket<Ipv6SocketAddr, SocketRuntime>;

    pub type Ipv4Datagram =
        crate::net::socket::uring::datagram::DatagramSocket<Ipv4SocketAddr, SocketRuntime>;
    pub type Ipv6Datagram =
        crate::net::socket::uring::datagram::DatagramSocket<Ipv6SocketAddr, SocketRuntime>;

    pub struct SocketRuntime;
    impl crate::net::socket::uring::runtime::Runtime for SocketRuntime {
        // Assign an IO-Uring instance for newly created socket
        fn io_uring() -> Arc<IoUring> {
            crate::io_uring::MULTITON.get_uring()
        }

        // Disattach IO-Uring instance with closed socket
        fn disattach_io_uring(fd: usize, uring: Arc<IoUring>) {
            crate::io_uring::MULTITON.disattach_uring(fd, uring);
        }
    }
}
