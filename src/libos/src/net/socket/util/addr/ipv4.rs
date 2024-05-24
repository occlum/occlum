use crate::net::{Addr, Domain};
use std::any::Any;
use std::fmt::{self, Debug};

use super::{CSockAddr, SockAddr};
use crate::prelude::*;

/// An IPv4 socket address, consisting of an IPv4 address and a port.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ipv4SocketAddr {
    ip: Ipv4Addr,
    port: u16,
}

impl Addr for Ipv4SocketAddr {
    fn domain() -> Domain {
        Domain::INET
    }

    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        if c_addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
            return_errno!(EINVAL, "address length is too large");
        }

        // The c_addr_len is certainly not smaller than the length of IN_ADDR_ANY.
        // https://en.wikipedia.org/wiki/IPv4
        if c_addr_len < std::mem::size_of::<libc::sockaddr_in>() {
            return_errno!(EINVAL, "address length is too small");
        }
        // Safe to convert from sockaddr_storage to sockaddr_in
        let c_addr = unsafe { std::mem::transmute(c_addr) };
        Self::from_c(c_addr)
    }

    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        let c_in_addr = self.to_c();
        c_in_addr.to_c_storage()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_default(&self) -> bool {
        let inaddr_any = Self::default();
        *self == inaddr_any
    }
}

impl Ipv4SocketAddr {
    pub fn new(ip: Ipv4Addr, port: u16) -> Self {
        Self { ip, port }
    }

    pub fn from_c(c_addr: &libc::sockaddr_in) -> Result<Self> {
        if c_addr.sin_family != libc::AF_INET as libc::sa_family_t {
            return_errno!(EINVAL, "an ipv4 address is expected");
        }
        Ok(Self {
            port: u16::from_be(c_addr.sin_port),
            ip: Ipv4Addr::from_c(&c_addr.sin_addr),
        })
    }

    pub fn to_c(&self) -> libc::sockaddr_in {
        libc::sockaddr_in {
            sin_family: libc::AF_INET as _,
            sin_port: self.port.to_be(),
            sin_addr: self.ip.to_c(),
            sin_zero: [0; 8],
        }
    }

    pub fn to_raw(&self) -> SockAddr {
        let (storage, len) = self.to_c_storage();
        SockAddr::from_c_storage(&storage, len)
    }

    pub fn ip(&self) -> &Ipv4Addr {
        &self.ip
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_ip(&mut self, new_ip: Ipv4Addr) {
        self.ip = new_ip;
    }

    pub fn set_port(&mut self, new_port: u16) {
        self.port = new_port;
    }
}

impl Default for Ipv4SocketAddr {
    fn default() -> Self {
        let addr = Ipv4Addr::new(0, 0, 0, 0);
        Self::new(addr, 0)
    }
}

/// An Ipv4 address.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Ipv4Addr([u8; 4] /* big endian */);

impl Ipv4Addr {
    /// Creates a new IPv4 address of `a.b.c.d`.
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    /// Creates a new IPv4 address from its C counterpart.
    pub fn from_c(c_addr: &libc::in_addr) -> Self {
        Self(c_addr.s_addr.to_ne_bytes())
    }

    /// Return the C counterpart.
    pub fn to_c(&self) -> libc::in_addr {
        libc::in_addr {
            s_addr: u32::from_ne_bytes(self.0),
        }
    }

    /// Return the four digits that make up the address.
    ///
    /// Assuming the address is `a.b.c.d`, the returned value would be `[a, b, c, d]`.
    pub fn octets(&self) -> &[u8; 4] {
        &self.0
    }
}

impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d] = *self.octets();
        write!(f, "Ipv4Addr ({}.{}.{}.{})", &a, &b, &c, &d)
    }
}
