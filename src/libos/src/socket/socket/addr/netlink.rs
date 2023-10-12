use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::any::Any;
use std::fmt::Debug;

use super::{Addr, CSockAddr, Domain};
use crate::prelude::*;
use libc::sockaddr_nl;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NetlinkSocketAddr {
    family: NetlinkFamily,
    pid: u32, // port id
    groups: u32,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u16)]
pub enum NetlinkFamily {
    NETLINK_ROUTE = 0,     /* Routing/device hook */
    NETLINK_UNUSED = 1,    /* Unused number */
    NETLINK_USERSOCK = 2,  /* Reserved for user mode socket protocols */
    NETLINK_FIREWALL = 3,  /* Unused number, formerly ip_queue */
    NETLINK_SOCK_DIAG = 4, /* socket monitoring */
    NETLINK_NFLOG = 5,     /* netfilter/iptables ULOG */
    NETLINK_XFRM = 6,      /* ipsec */
    NETLINK_SELINUX = 7,   /* SELinux event notifications */
    NETLINK_ISCSI = 8,     /* Open-iSCSI */
    NETLINK_AUDIT = 9,     /* auditing */
    NETLINK_FIB_LOOKUP = 10,
    NETLINK_CONNECTOR = 11,
    NETLINK_NETFILTER = 12, /* netfilter subsystem */
    NETLINK_IP6_FW = 13,
    NETLINK_DNRTMSG = 14,        /* DECnet routing messages */
    NETLINK_KOBJECT_UEVENT = 15, /* Kernel messages to userspace */
    NETLINK_GENERIC = 16,
    /* leave room for NETLINK_DM (DM Events) */
    NETLINK_SCSITRANSPORT = 18, /* SCSI Transports */
    NETLINK_ECRYPTFS = 19,
    NETLINK_RDMA = 20,
    NETLINK_CRYPTO = 21, /* Crypto layer */
    NETLINK_SMC = 22,    /* SMC monitoring */
}

impl NetlinkSocketAddr {
    pub fn new(netlink_family: NetlinkFamily, port: u32, group_id: u32) -> Self {
        Self {
            family: netlink_family,
            pid: port,
            groups: group_id,
        }
    }

    pub fn from_c(c_addr: &libc::sockaddr_nl) -> Result<Self> {
        Ok(Self {
            family: NetlinkFamily::try_from(c_addr.nl_family)
                .map_err(|_| errno!(EINVAL, "invalid or unsupported netlink family"))?,
            pid: c_addr.nl_pid,
            groups: c_addr.nl_groups,
        })
    }

    pub fn to_c(&self) -> libc::sockaddr_nl {
        let c_addr = sockaddr_nl_t {
            nl_family: self.family as _,
            nl_pad: 0,
            nl_pid: self.pid,
            nl_groups: self.groups,
        };
        let c_addr: libc::sockaddr_nl = unsafe { std::mem::transmute(c_addr) };
        c_addr
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Addr for NetlinkSocketAddr {
    fn domain() -> Domain {
        Domain::Netlink
    }

    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        if c_addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
            return_errno!(EINVAL, "address length is too large");
        }
        // Safe to convert from sockaddr_storage to sockaddr_in
        let c_addr: &sockaddr_nl = unsafe { std::mem::transmute(c_addr) };
        Self::from_c(c_addr)
    }

    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        let c_addr = self.to_c();
        (c_addr, std::mem::size_of::<libc::sockaddr_nl>()).to_c_storage()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_default(&self) -> bool {
        let netlink_default = Self::default();
        *self == netlink_default
    }
}

impl Default for NetlinkSocketAddr {
    fn default() -> Self {
        Self::new(NetlinkFamily::NETLINK_ROUTE, 0, 0)
    }
}

// Internal use for libc::sockaddr_nl
#[repr(C)]
struct sockaddr_nl_t {
    nl_family: u16,
    nl_pad: u16,
    nl_pid: u32,
    nl_groups: u32,
}
