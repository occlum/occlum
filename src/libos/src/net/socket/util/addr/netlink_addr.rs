use crate::net::{Addr, Domain};
use core::mem::MaybeUninit;
use std::any::Any;
use std::fmt::{self, Debug};

use super::{CSockAddr, SockAddr};
use crate::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NetlinkSocketAddr {
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
    pub fn new(pid: pid_t, groups: u32) -> Self {
        Self { pid, groups }
    }

    // since netlink is used for communicating with kernel, address's byte order is native
    pub fn from_c(c_addr: &libc::sockaddr_nl) -> Result<Self> {
        if c_addr.nl_family != libc::AF_NETLINK as libc::sa_family_t {
            return_errno!(EINVAL, "a netlink address is expected");
        }
        Ok(Self {
            pid: c_addr.nl_pid,
            groups: c_addr.nl_groups,
        })
    }

    // since netlink is used for communicating with kernel, address's byte order is native
    pub fn to_c(&self) -> libc::sockaddr_nl {
        let mut sockaddr_nl = unsafe { MaybeUninit::<libc::sockaddr_nl>::uninit().assume_init() };

        sockaddr_nl.nl_family = libc::AF_NETLINK as _;
        sockaddr_nl.nl_pid = self.pid;
        sockaddr_nl.nl_groups = self.groups;

        return sockaddr_nl;
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
        Self::new(0, 0)
    }
}
