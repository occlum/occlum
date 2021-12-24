use std::mem::{self, MaybeUninit};

use crate::prelude::*;

pub use async_io::socket::{
    Addr, CSockAddr, Domain, Ipv4Addr, Ipv4SocketAddr, Ipv6SocketAddr, UnixAddr,
};
use num_enum::IntoPrimitive;

#[derive(Clone, Debug, PartialEq)]
pub enum AnyAddr {
    Ipv4(Ipv4SocketAddr),
    Ipv6(Ipv6SocketAddr),
    Unix(UnixAddr),
    Unspec,
}

impl AnyAddr {
    pub fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        let any_addr = match c_addr.ss_family as _ {
            libc::AF_INET => {
                let ipv4_addr = Ipv4SocketAddr::from_c_storage(c_addr, c_addr_len)?;
                Self::Ipv4(ipv4_addr)
            }
            libc::AF_INET6 => {
                let ipv6_addr = Ipv6SocketAddr::from_c_storage(c_addr, c_addr_len)?;
                Self::Ipv6(ipv6_addr)
            }
            libc::AF_UNIX | libc::AF_LOCAL => {
                let unix_addr = UnixAddr::from_c_storage(c_addr, c_addr_len)?;
                Self::Unix(unix_addr)
            }
            libc::AF_UNSPEC => Self::Unspec,
            _ => {
                return_errno!(EINVAL, "unsupported or invalid address family");
            }
        };
        Ok(any_addr)
    }

    pub fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        match self {
            Self::Ipv4(ipv4_addr) => ipv4_addr.to_c_storage(),
            Self::Ipv6(ipv6_addr) => ipv6_addr.to_c_storage(),
            Self::Unix(unix_addr) => unix_addr.to_c_storage(),
            Self::Unspec => {
                let mut sockaddr_storage =
                    unsafe { MaybeUninit::<libc::sockaddr_storage>::uninit().assume_init() };
                sockaddr_storage.ss_family = libc::AF_UNSPEC as _;
                (sockaddr_storage, mem::size_of::<libc::sa_family_t>())
            }
        }
    }

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

    pub fn to_ipv4(&self) -> Result<&Ipv4SocketAddr> {
        match self {
            Self::Ipv4(ipv4_addr) => Ok(ipv4_addr),
            _ => return_errno!(EAFNOSUPPORT, "not ipv4 address"),
        }
    }

    pub fn to_ipv6(&self) -> Result<&Ipv6SocketAddr> {
        match self {
            Self::Ipv6(ipv6_addr) => Ok(ipv6_addr),
            _ => return_errno!(EAFNOSUPPORT, "not ipv6 address"),
        }
    }

    pub fn to_unix(&self) -> Result<&UnixAddr> {
        match self {
            Self::Unix(unix_addr) => Ok(unix_addr),
            _ => return_errno!(EAFNOSUPPORT, "not unix address"),
        }
    }

    pub fn is_unspec(&self) -> bool {
        match self {
            Self::Unspec => true,
            _ => false,
        }
    }
}
