use crate::net::{Addr, Domain};
use core::mem::MaybeUninit;
use std::any::Any;
use std::fmt::{self, Debug};

use super::{CSockAddr, SockAddr};
use crate::prelude::*;

type NetlinkFamily = u16;
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NetlinkSocketAddr {
    family: NetlinkFamily,
    pid: pid_t,
    groups: u32,
}

impl Addr for NetlinkSocketAddr {
    fn domain() -> Domain {
        Domain::NETLINK
    }

    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        if c_addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
            return_errno!(EINVAL, "address length is too large");
        }

        if c_addr_len < std::mem::size_of::<libc::sockaddr_nl>() {
            return_errno!(EINVAL, "address length is too small");
        }
        // Safe to convert from sockaddr_storage to sockaddr_nl
        let c_addr = unsafe { std::mem::transmute(c_addr) };
        Self::from_c(c_addr)
    }

    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        let c_nl_addr = self.to_c();
        (c_nl_addr, std::mem::size_of::<libc::sockaddr_nl>()).to_c_storage()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_default(&self) -> bool {
        let nladdr_any_init = Self::default();
        *self == nladdr_any_init
    }
}

impl NetlinkSocketAddr {
    pub fn new(family: NetlinkFamily, pid: pid_t, groups: u32) -> Self {
        Self {
            family,
            pid,
            groups,
        }
    }

    // since netlink is used for communicating with kernel, address's byte order is native
    pub fn from_c(c_addr: &libc::sockaddr_nl) -> Result<Self> {
        if c_addr.nl_family != libc::AF_NETLINK as libc::sa_family_t {
            return_errno!(EINVAL, "a netlink address is expected");
        }
        Ok(Self {
            family: c_addr.nl_family,
            pid: c_addr.nl_pid,
            groups: c_addr.nl_groups,
        })
    }

    // since netlink is used for communicating with kernel, address's byte order is native
    pub fn to_c(&self) -> libc::sockaddr_nl {
        #[repr(C)]
        struct sockaddr_nl_t {
            nl_family: u16,
            nl_pad: u16,
            nl_pid: u32,
            nl_groups: u32,
        }

        let c_addr = sockaddr_nl_t {
            nl_family: self.family,
            nl_pad: 0,
            nl_pid: self.pid,
            nl_groups: self.groups,
        };
        let c_addr: libc::sockaddr_nl = unsafe { std::mem::transmute(c_addr) };
        c_addr
    }

    pub fn to_raw(&self) -> SockAddr {
        let (storage, len) = self.to_c_storage();
        SockAddr::from_c_storage(&storage, len)
    }

    pub fn pid(&self) -> &pid_t {
        &self.pid
    }

    pub fn groups(&self) -> u32 {
        self.groups
    }

    pub fn set_pid(&mut self, new_pid: pid_t) {
        self.pid = new_pid
    }

    pub fn set_groups(&mut self, new_groups: u32) {
        self.groups = new_groups
    }

    pub fn add_group(&mut self, new_group: u32) {
        self.groups |= new_group
    }
}

impl Default for NetlinkSocketAddr {
    fn default() -> Self {
        Self::new(libc::AF_NETLINK as libc::sa_family_t, 0, 0)
    }
}
