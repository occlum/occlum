use std::any::Any;
use std::fmt::Debug;

use super::SockAddr;
use super::{Addr, CSockAddr, Domain};
use crate::prelude::*;
use libc::in6_addr;
use libc::sockaddr_in6;

pub use std::net::Ipv6Addr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ipv6SocketAddr {
    ip: Ipv6Addr,
    port: u16,
    flowinfo: u32,
    scope_id: u32,
}

impl Addr for Ipv6SocketAddr {
    fn domain() -> Domain {
        Domain::INET6
    }

    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        if c_addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
            return_errno!(EINVAL, "address length is too large");
        }

        if c_addr_len < std::mem::size_of::<sockaddr_in6>() {
            return_errno!(EINVAL, "address length is too small");
        }
        // Safe to convert from sockaddr_storage to sockaddr_in
        let c_addr: &sockaddr_in6 = unsafe { std::mem::transmute(c_addr) };
        Self::from_c(c_addr)
    }

    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        let c_addr = self.to_c();
        (c_addr, std::mem::size_of::<libc::sockaddr_in6>()).to_c_storage()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_default(&self) -> bool {
        let in6addr_any_init = Self::default();
        *self == in6addr_any_init
    }
}

impl Ipv6SocketAddr {
    pub fn new(ip: Ipv6Addr, port: u16, flowinfo: u32, scope_id: u32) -> Self {
        Self {
            ip,
            port,
            flowinfo,
            scope_id,
        }
    }

    pub fn from_c(c_addr: &libc::sockaddr_in6) -> Result<Self> {
        if c_addr.sin6_family != libc::AF_INET6 as libc::sa_family_t {
            return_errno!(EINVAL, "an ipv6 address is expected");
        }
        Ok(Self {
            port: u16::from_be(c_addr.sin6_port),
            ip: Ipv6Addr::from(c_addr.sin6_addr.s6_addr),
            flowinfo: u32::from_be(c_addr.sin6_flowinfo),
            scope_id: u32::from_be(c_addr.sin6_scope_id),
        })
    }

    pub fn to_c(&self) -> libc::sockaddr_in6 {
        let in6_addr = in6_addr {
            s6_addr: self.ip.octets(),
        };
        libc::sockaddr_in6 {
            sin6_family: libc::AF_INET6 as _,
            sin6_port: self.port.to_be(),
            sin6_addr: in6_addr,
            sin6_flowinfo: self.flowinfo.to_be(),
            sin6_scope_id: self.flowinfo.to_be(),
        }
    }

    pub fn to_raw(&self) -> SockAddr {
        let (storage, len) = self.to_c_storage();
        SockAddr::from_c_storage(&storage, len)
    }

    pub fn ip(&self) -> &Ipv6Addr {
        &self.ip
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_ip(&mut self, new_ip: Ipv6Addr) {
        self.ip = new_ip;
    }

    pub fn set_port(&mut self, new_port: u16) {
        self.port = new_port;
    }
}

impl Default for Ipv6SocketAddr {
    fn default() -> Self {
        let addr = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0);
        Self::new(addr, 0, 0, 0)
    }
}
