pub use async_io::socket::{Addr, CSockAddr, Domain, Ipv4Addr, Ipv4SocketAddr, UnixAddr};

#[derive(Clone, Debug, PartialEq)]
pub enum AnyAddr {
    Ipv4(Ipv4SocketAddr),
    Unix(UnixAddr),
}

impl AnyAddr {
    pub fn as_ipv4(&self) -> Option<&Ipv4SocketAddr> {
        match self {
            Self::Ipv4(ipv4_addr) => Some(ipv4_addr),
            _ => None,
        }
    }

    pub fn as_unix(&self) -> Option<&UnixAddr> {
        match self {
            Self::Unix(unix_addr) => Some(unix_addr),
            _ => None,
        }
    }
}
