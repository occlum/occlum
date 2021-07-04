use self::impls::{Ipv4Stream, UnixStream};
use crate::fs::{AccessMode, Events, Poller, StatusFlags};
use crate::net::{Addr, AnyAddr, Domain, Ipv4SocketAddr, UnixAddr};
use crate::prelude::*;

#[derive(Debug)]
pub struct SocketFile {
    socket: AnySocket,
}

#[derive(Debug)]
enum AnySocket {
    UnixStream(UnixStream),
    Ipv4Stream(Ipv4Stream),
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
        //apply_fn_on_any_socket!(&self.socket, |socket| { socket.status_flags() })
        todo!("")
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        //apply_fn_on_any_socket!(&self.socket, |socket| { socket.set_status_flags(new_flags) })
        todo!("")
    }

    pub fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.poll_by(mask, poller) })
    }
}

// Implement socket-specific methods
impl SocketFile {
    pub fn new(domain: Domain, is_stream: bool) -> Result<Self> {
        if is_stream {
            let any_socket = match domain {
                Domain::Ipv4 => {
                    let ipv4_stream = Ipv4Stream::new()?;
                    AnySocket::Ipv4Stream(ipv4_stream)
                }
                Domain::Unix => {
                    let unix_stream = UnixStream::new()?;
                    AnySocket::UnixStream(unix_stream)
                }
                _ => {
                    return_errno!(EINVAL, "not support IPv6, yet");
                }
            };
            let new_self = Self { socket: any_socket };
            Ok(new_self)
        } else {
            return_errno!(EINVAL, "not support non-stream sockets, yet");
        }
    }

    pub fn domain(&self) -> Domain {
        apply_fn_on_any_socket!(&self.socket, |socket| { socket.domain() })
    }

    pub fn is_stream(&self) -> bool {
        matches!(&self.socket, AnySocket::Ipv4Stream(_) | AnySocket::UnixStream(_))
    }

    pub async fn connect(&self, addr: &AnyAddr) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let ip_addr = addr
                    .as_ipv4()
                    .ok_or_else(|| errno!(EAFNOSUPPORT, "not ipv4 address"))?;
                ipv4_stream.connect(ip_addr).await
            }
            AnySocket::UnixStream(unix_stream) => {
                let unix_addr = addr
                    .as_unix()
                    .ok_or_else(|| errno!(EAFNOSUPPORT, "not unix address"))?;
                unix_stream.connect(unix_addr).await
            }
            _ => {
                return_errno!(EINVAL, "connect is not supported");
            }
        }
    }

    pub fn bind(&self, addr: &AnyAddr) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let ip_addr = addr
                    .as_ipv4()
                    .ok_or_else(|| errno!(EAFNOSUPPORT, "not ipv4 address"))?;
                ipv4_stream.bind(ip_addr)
            }
            AnySocket::UnixStream(unix_stream) => {
                let unix_addr = addr
                    .as_unix()
                    .ok_or_else(|| errno!(EAFNOSUPPORT, "not unix address"))?;
                unix_stream.bind(unix_addr)
            }
            _ => {
                return_errno!(EINVAL, "bind is not supported");
            }
        }
    }

    pub fn listen(&self, backlog: u32) -> Result<()> {
        match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => ipv4_stream.listen(backlog),
            AnySocket::UnixStream(unix_stream) => unix_stream.listen(backlog),
            _ => {
                return_errno!(EINVAL, "listen is not supported");
            }
        }
    }

    pub async fn accept(&self) -> Result<Self> {
        let accepted_any_socket = match &self.socket {
            AnySocket::Ipv4Stream(ipv4_stream) => {
                let accepted_ipv4_stream = ipv4_stream.accept().await?;
                AnySocket::Ipv4Stream(accepted_ipv4_stream)
            }
            AnySocket::UnixStream(unix_stream) => {
                let accepted_unix_stream = unix_stream.accept().await?;
                AnySocket::UnixStream(accepted_unix_stream)
            }
            _ => {
                return_errno!(EINVAL, "listen is not supported");
            }
        };
        let accepted_socket_file = SocketFile {
            socket: accepted_any_socket,
        };
        Ok(accepted_socket_file)
    }
}

mod impls {
    use super::*;
    use io_uring_callback::IoUring;

    pub type Ipv4Stream = host_socket::StreamSocket<Ipv4SocketAddr, SocketRuntime>;
    // TODO: UnixStream cannot be simply re-exported from host_socket.
    // There are two reasons. First, there needs to be some translation between LibOS
    // and host paths. Second, we need two types of unix domain sockets: the trusted one that
    // is implemented inside LibOS and the untrusted one that is implemented by host OS.
    pub type UnixStream = host_socket::StreamSocket<UnixAddr, SocketRuntime>;

    pub struct SocketRuntime;

    impl host_socket::Runtime for SocketRuntime {
        fn io_uring() -> &'static IoUring {
            &*crate::io_uring::SINGLETON
        }
    }
}
