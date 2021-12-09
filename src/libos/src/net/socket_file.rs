use async_io::ioctl::IoctlCmd;
use async_io::socket::{RecvFlags, SendFlags, Shutdown};

use self::impls::{Ipv4Datagram, Ipv4Stream, Ipv6Stream, UnixDatagram};
use super::unix::trusted::Stream as TrustedStream;
use super::unix::UnixStream;
use crate::fs::{AccessMode, Events, Observer, Poller, StatusFlags};
use crate::net::{Addr, AnyAddr, Domain, Ipv4SocketAddr, Ipv6SocketAddr, UnixAddr};
use crate::prelude::*;
use num_enum::{IntoPrimitive, TryFromPrimitive};

pub use self::impls::UntrustedUnixStream;

#[derive(Debug)]
pub struct SocketFile {
    socket: AnySocket,
}

#[derive(Debug)]
enum AnySocket {
    UnixStream(UnixStream), // for general usage
    Ipv4Stream(Ipv4Stream),
    Ipv6Stream(Ipv6Stream),
    UnixDatagram(UnixDatagram),
    Ipv4Datagram(Ipv4Datagram),
    TrustedUDS(TrustedStream), // for socket pair use only
}

/* Standard well-defined IP protocols.  */
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
pub enum SocketProtocol {
    IPPROTO_IP = 0,        /* Dummy protocol for TCP.  */
    IPPROTO_ICMP = 1,      /* Internet Control Message Protocol.  */
    IPPROTO_IGMP = 2,      /* Internet Group Management Protocol. */
    IPPROTO_IPIP = 4,      /* IPIP tunnels (older KA9Q tunnels use 94).  */
    IPPROTO_TCP = 6,       /* Transmission Control Protocol.  */
    IPPROTO_EGP = 8,       /* Exterior Gateway Protocol.  */
    IPPROTO_PUP = 12,      /* PUP protocol.  */
    IPPROTO_UDP = 17,      /* User Datagram Protocol.  */
    IPPROTO_IDP = 22,      /* XNS IDP protocol.  */
    IPPROTO_TP = 29,       /* SO Transport Protocol Class 4.  */
    IPPROTO_DCCP = 33,     /* Datagram Congestion Control Protocol.  */
    IPPROTO_IPV6 = 41,     /* IPv6 header.  */
    IPPROTO_RSVP = 46,     /* Reservation Protocol.  */
    IPPROTO_GRE = 47,      /* General Routing Encapsulation.  */
    IPPROTO_ESP = 50,      /* encapsulating security payload.  */
    IPPROTO_AH = 51,       /* authentication header.  */
    IPPROTO_MTP = 92,      /* Multicast Transport Protocol.  */
    IPPROTO_BEETPH = 94,   /* IP option pseudo header for BEET.  */
    IPPROTO_ENCAP = 98,    /* Encapsulation Header.  */
    IPPROTO_PIM = 103,     /* Protocol Independent Multicast.  */
    IPPROTO_COMP = 108,    /* Compression Header Protocol.  */
    IPPROTO_SCTP = 132,    /* Stream Control Transmission Protocol.  */
    IPPROTO_UDPLITE = 136, /* UDP-Lite protocol.  */
    IPPROTO_MPLS = 137,    /* MPLS in IP.  */
    IPPROTO_RAW = 255,     /* Raw IP packets.  */
    IPPROTO_MAX,
}

// Apply a function to all variants of AnySocket enum.
macro_rules! apply_fn_on_any_socket {
    ($any_socket:expr, |$socket:ident| { $($fn_body:tt)* }) => {{
        let any_socket: &AnySocket = $any_socket;
        match any_socket {
            AnySocket::UnixStream($socket) => {
                $($fn_body)*
            }
            AnySocket::Ipv4Stream($socket) => {
                $($fn_body)*
            }
            AnySocket::Ipv6Stream($socket) => {
                $($fn_body)*
            }
            AnySocket::UnixDatagram($socket) => {
                $($fn_body)*
            }
            AnySocket::Ipv4Datagram($socket) => {
                $($fn_body)*
            }
            AnySocket::TrustedUDS($socket) => {
                $($fn_body)*
            }
        }
    }}
}

// Implement the common methods required by FileHandle
impl SocketFile {
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.read(buf).await })
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.readv(bufs).await })
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.write(buf).await })
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.writev(bufs).await })
    }

    pub fn access_mode(&self) -> AccessMode {
        // We consider all sockets both readable and writable
        AccessMode::O_RDWR
    }

    pub fn status_flags(&self) -> StatusFlags {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.status_flags() })
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        apply_fn_on_any_socket!(&self.socket, |socket| {
            socket.set_status_flags(new_flags)
        })
    }

    pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.poll(mask, poller) })
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        apply_fn_on_any_socket!(&self.socket, |socket| {
            socket.register_observer(observer, mask)
        })
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        apply_fn_on_any_socket!(&self.socket, |socket| {
            socket.unregister_observer(observer)
        })
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.ioctl(cmd) })
    }
}

// Implement socket-specific methods
impl SocketFile {
    pub fn new(
        domain: Domain,
        protocol: SocketProtocol,
        is_stream: bool,
        nonblocking: bool,
    ) -> Result<Self> {
        if protocol != SocketProtocol::IPPROTO_IP && protocol != SocketProtocol::IPPROTO_TCP {
            return_errno!(EINVAL, "unsupported protocol");
        }

        if is_stream {
            let any_socket = match domain {
                Domain::Ipv4 => {
                    let ipv4_stream = Ipv4Stream::new(nonblocking)?;
                    AnySocket::Ipv4Stream(ipv4_stream)
                }
                Domain::Ipv6 => {
                    let ipv6_stream = Ipv6Stream::new(nonblocking)?;
                    AnySocket::Ipv6Stream(ipv6_stream)
                }
                Domain::Unix => {
                    let unix_stream = UnixStream::new_trusted(nonblocking);
                    AnySocket::UnixStream(unix_stream)
                }
            };
            let new_self = Self { socket: any_socket };
            Ok(new_self)
        } else {
            let any_socket = match domain {
                Domain::Ipv4 => {
                    let ipv4_datagram = Ipv4Datagram::new(nonblocking)?;
                    AnySocket::Ipv4Datagram(ipv4_datagram)
                }
                Domain::Unix => {
                    let unix_datagram = UnixDatagram::new(nonblocking)?;
                    AnySocket::UnixDatagram(unix_datagram)
                }
                _ => {
                    return_errno!(EINVAL, "not support IPv6, yet");
                }
            };
            let new_self = Self { socket: any_socket };
            Ok(new_self)
        }
    }

    pub fn new_pair(is_stream: bool, nonblocking: bool) -> Result<(Self, Self)> {
        if is_stream {
            // Use trusted Unix domain sockets as stream socket pair
            let (stream1, stream2) = TrustedStream::socketpair(nonblocking)?;
            let sock_file1 = Self {
                socket: AnySocket::TrustedUDS(stream1),
            };
            let sock_file2 = Self {
                socket: AnySocket::TrustedUDS(stream2),
            };
            Ok((sock_file1, sock_file2))
        } else {
            let (datagram1, datagram2) = UnixDatagram::new_pair(nonblocking)?;
            let sock_file1 = Self {
                socket: AnySocket::UnixDatagram(datagram1),
            };
            let sock_file2 = Self {
                socket: AnySocket::UnixDatagram(datagram2),
            };
            Ok((sock_file1, sock_file2))
        }
    }

    pub fn domain(&self) -> Domain {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.domain() })
    }

    pub fn is_stream(&self) -> bool {
        matches!(
            &self.socket,
            AnySocket::Ipv4Stream(_) | AnySocket::UnixStream(_) | AnySocket::TrustedUDS(_)
        )
    }

    pub async fn connect(&self, addr: &AnyAddr) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let ip_addr = addr.to_ipv4()?;
                ipv4_stream.connect(ip_addr).await
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let ip_addr = addr.to_ipv6()?;
                ipv6_stream.connect(ip_addr).await
            }
            AnySocket::UnixStream(unix_stream) => {
                let unix_addr = addr.to_trusted_unix()?;
                unix_stream.connect(unix_addr).await
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let ip_addr = if addr.is_unspec() {
                    None
                } else {
                    Some(addr.to_ipv4()?)
                };
                ipv4_datagram.connect(ip_addr).await
            }
            AnySocket::UnixDatagram(unix_datagram) => {
                let unix_addr = if addr.is_unspec() {
                    None
                } else {
                    Some(addr.to_unix()?)
                };
                unix_datagram.connect(unix_addr).await
            }
            _ => {
                return_errno!(EINVAL, "connect is not supported");
            }
        }
    }

    pub fn bind(&self, addr: &mut AnyAddr) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let ip_addr = addr.to_ipv4()?;
                ipv4_stream.bind(ip_addr)
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let ip_addr = addr.to_ipv6()?;
                ipv6_stream.bind(ip_addr)
            }
            AnySocket::UnixStream(unix_stream) => {
                let mut trusted_addr = addr.to_trusted_unix_mut()?;
                unix_stream.bind(trusted_addr)
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let ip_addr = addr.to_ipv4()?;
                ipv4_datagram.bind(ip_addr)
            }
            AnySocket::UnixDatagram(unix_datagram) => {
                let unix_addr = addr.to_unix()?;
                unix_datagram.bind(unix_addr)
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
            AnySocket::UnixStream(unix_stream) => unix_stream.listen(backlog),
            _ => {
                return_errno!(EINVAL, "listen is not supported");
            }
        }
    }

    pub async fn accept(&self, nonblocking: bool) -> Result<Self> {
        let accepted_any_socket = match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let accepted_ipv4_stream = ipv4_stream.accept(nonblocking).await?;
                AnySocket::Ipv4Stream(accepted_ipv4_stream)
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let accepted_ipv6_stream = ipv6_stream.accept(nonblocking).await?;
                AnySocket::Ipv6Stream(accepted_ipv6_stream)
            }
            AnySocket::UnixStream(unix_stream) => {
                let accepted_unix_stream = unix_stream.accept(nonblocking).await?;
                AnySocket::UnixStream(accepted_unix_stream)
            }
            _ => {
                return_errno!(EINVAL, "accept is not supported");
            }
        };
        let accepted_socket_file = SocketFile {
            socket: accepted_any_socket,
        };
        Ok(accepted_socket_file)
    }

    pub async fn recvfrom(
        &self,
        buf: &mut [u8],
        flags: RecvFlags,
    ) -> Result<(usize, Option<AnyAddr>)> {
        self.recvmsg(&mut [buf], flags).await
    }

    pub async fn recvmsg(
        &self,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
    ) -> Result<(usize, Option<AnyAddr>)> {
        // TODO: support msg_flags and msg_control
        Ok(match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let bytes_recv = ipv4_stream.recvmsg(bufs, flags).await?;
                (bytes_recv, None)
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                let bytes_recv = ipv6_stream.recvmsg(bufs, flags).await?;
                (bytes_recv, None)
            }
            AnySocket::UnixStream(unix_stream) => {
                let bytes_recv = unix_stream.recvmsg(bufs, flags).await?;
                (bytes_recv, None)
            }
            AnySocket::TrustedUDS(trusted_stream) => {
                let bytes_recv = trusted_stream.recvmsg(bufs, flags).await?;
                (bytes_recv, None)
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let (bytes_recv, addr_recv) = ipv4_datagram.recvmsg(bufs, flags).await?;
                (bytes_recv, Some(AnyAddr::Ipv4(addr_recv)))
            }
            AnySocket::UnixDatagram(unix_datagram) => {
                let (bytes_recv, addr_recv) = unix_datagram.recvmsg(bufs, flags).await?;
                (bytes_recv, Some(AnyAddr::Unix(addr_recv)))
            }
            _ => {
                return_errno!(EINVAL, "recvfrom is not supported");
            }
        })
    }

    pub async fn sendto(
        &self,
        buf: &[u8],
        addr: Option<AnyAddr>,
        flags: SendFlags,
    ) -> Result<usize> {
        self.sendmsg(&[buf], addr, flags).await
    }

    pub async fn sendmsg(
        &self,
        bufs: &[&[u8]],
        addr: Option<AnyAddr>,
        flags: SendFlags,
    ) -> Result<usize> {
        let res = match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                if addr.is_some() {
                    return_errno!(EISCONN, "addr should be none");
                }
                ipv4_stream.sendmsg(bufs, flags).await
            }
            AnySocket::Ipv6Stream(ipv6_stream) => {
                if addr.is_some() {
                    return_errno!(EISCONN, "addr should be none");
                }
                ipv6_stream.sendmsg(bufs, flags).await
            }
            AnySocket::UnixStream(unix_stream) => {
                if addr.is_some() {
                    return_errno!(EISCONN, "addr should be none");
                }
                unix_stream.sendmsg(bufs, flags).await
            }
            AnySocket::TrustedUDS(trusted_stream) => {
                if addr.is_some() {
                    return_errno!(EISCONN, "addr should be none");
                }
                trusted_stream.sendmsg(bufs, flags).await
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => {
                let ip_addr = if let Some(addr) = addr.as_ref() {
                    Some(addr.to_ipv4()?)
                } else {
                    None
                };
                ipv4_datagram.sendmsg(bufs, ip_addr, flags).await
            }
            AnySocket::UnixDatagram(unix_datagram) => {
                let unix_addr = if let Some(addr) = addr.as_ref() {
                    Some(addr.to_unix()?)
                } else {
                    None
                };
                unix_datagram.sendmsg(bufs, unix_addr, flags).await
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
            AnySocket::UnixStream(unix_stream) => unix_stream.addr()?,
            AnySocket::TrustedUDS(trusted_stream) => AnyAddr::TrustedUnix(trusted_stream.addr()?),
            AnySocket::Ipv4Datagram(ipv4_datagram) => AnyAddr::Ipv4(ipv4_datagram.addr()?),
            AnySocket::UnixDatagram(unix_datagram) => AnyAddr::Unix(unix_datagram.addr()?),
            _ => {
                return_errno!(EINVAL, "addr is not supported");
            }
        })
    }

    pub fn peer_addr(&self) -> Result<AnyAddr> {
        Ok(match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => AnyAddr::Ipv4(ipv4_stream.peer_addr()?),
            AnySocket::Ipv6Stream(ipv6_stream) => AnyAddr::Ipv6(ipv6_stream.peer_addr()?),
            AnySocket::UnixStream(unix_stream) => unix_stream.peer_addr()?,
            AnySocket::TrustedUDS(trusted_stream) => {
                AnyAddr::TrustedUnix(trusted_stream.peer_addr()?)
            }
            AnySocket::Ipv4Datagram(ipv4_datagram) => AnyAddr::Ipv4(ipv4_datagram.peer_addr()?),
            AnySocket::UnixDatagram(unix_datagram) => AnyAddr::Unix(unix_datagram.peer_addr()?),
            _ => {
                return_errno!(EINVAL, "peer_addr is not supported");
            }
        })
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => ipv4_stream.shutdown(how),
            AnySocket::Ipv6Stream(ipv6_stream) => ipv6_stream.shutdown(how),
            AnySocket::UnixStream(unix_stream) => unix_stream.shutdown(how),
            _ => {
                return_errno!(EINVAL, "shutdown is not supported");
            }
        }
    }
}

mod impls {
    use super::*;
    use io_uring_callback::IoUring;

    pub type Ipv4Stream = host_socket::StreamSocket<Ipv4SocketAddr, SocketRuntime>;
    pub type Ipv6Stream = host_socket::StreamSocket<Ipv6SocketAddr, SocketRuntime>;
    // TODO: UnixStream cannot be simply re-exported from host_socket.
    // There are two reasons. First, there needs to be some translation between LibOS
    // and host paths. Second, we need two types of unix domain sockets: the trusted one that
    // is implemented inside LibOS and the untrusted one that is implemented by host OS.
    pub type UntrustedUnixStream = host_socket::StreamSocket<UnixAddr, SocketRuntime>;

    pub type Ipv4Datagram = host_socket::DatagramSocket<Ipv4SocketAddr, SocketRuntime>;
    pub type UnixDatagram = host_socket::DatagramSocket<UnixAddr, SocketRuntime>;

    pub struct SocketRuntime;

    impl host_socket::Runtime for SocketRuntime {
        fn io_uring() -> &'static IoUring {
            &*crate::io_uring::SINGLETON
        }
    }
}
