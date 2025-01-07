use crate::Domain;

use super::Addr;
use super::{CSockAddr, SockAddr};
use crate::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LinkLayerSocketAddr {
    protocol: u16,
    ifindex: i32,
    hatype: u16,
    pkttype: u8,
    halen: u8,
    addr: [u8; 8],
}

impl Addr for LinkLayerSocketAddr {
    fn domain() -> crate::Domain {
        Domain::PACKET
    }

    fn from_c_storage(
        c_addr: &sgx_trts::libc::sockaddr_storage,
        c_addr_len: usize,
    ) -> Result<Self> {
        if c_addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
            return_errno!(EINVAL, "address length is too large");
        }

        if c_addr_len < std::mem::size_of::<libc::sockaddr_ll>() {
            return_errno!(EINVAL, "address length is too small");
        }
        // Safe to convert from sockaddr_storage to sockaddr_ll
        let c_addr = unsafe { std::mem::transmute(c_addr) };
        Self::from_c(c_addr)
    }

    fn to_c_storage(&self) -> (sgx_trts::libc::sockaddr_storage, usize) {
        let c_ll_addr = self.to_c();
        (c_ll_addr, std::mem::size_of::<libc::sockaddr_ll>()).to_c_storage()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn is_default(&self) -> bool {
        let lladdr_any_init = Self::default();
        *self == lladdr_any_init
    }
}

impl LinkLayerSocketAddr {
    pub fn new(
        protocol: u16,
        ifindex: i32,
        hatype: u16,
        pkttype: u8,
        halen: u8,
        addr: [u8; 8],
    ) -> Self {
        Self {
            protocol,
            ifindex,
            hatype,
            pkttype,
            halen,
            addr,
        }
    }

    // only sll_protocol use big endian
    pub fn from_c(c_addr: &libc::sockaddr_ll) -> Result<Self> {
        if c_addr.sll_family != libc::AF_PACKET as libc::sa_family_t {
            return_errno!(EINVAL, "a packet address is expected")
        }
        Ok(Self {
            protocol: u16::from_be(c_addr.sll_protocol),
            ifindex: c_addr.sll_ifindex,
            hatype: c_addr.sll_hatype,
            pkttype: c_addr.sll_pkttype,
            halen: c_addr.sll_halen,
            addr: c_addr.sll_addr,
        })
    }

    pub fn to_c(&self) -> libc::sockaddr_ll {
        libc::sockaddr_ll {
            sll_family: libc::AF_PACKET as _,
            sll_protocol: self.protocol.to_be(),
            sll_ifindex: self.ifindex,
            sll_hatype: self.hatype,
            sll_pkttype: self.pkttype,
            sll_halen: self.halen,
            sll_addr: self.addr,
        }
    }

    pub fn to_raw(&self) -> SockAddr {
        let (storage, len) = self.to_c_storage();
        SockAddr::from_c_storage(&storage, len)
    }

    pub fn protocol(&self) -> u16 {
        self.protocol
    }

    pub fn ifindex(&self) -> i32 {
        self.ifindex
    }

    pub fn hatype(&self) -> u16 {
        self.hatype
    }

    pub fn pkttype(&self) -> u8 {
        self.pkttype
    }

    pub fn halen(&self) -> u8 {
        self.halen
    }

    pub fn addr(&self) -> &[u8; 8] {
        &self.addr
    }

    pub fn set_protocol(&mut self, new_protocol: u16) {
        self.protocol = new_protocol
    }

    pub fn set_ifindex(&mut self, new_ifindex: i32) {
        self.ifindex = new_ifindex
    }

    pub fn set_hatype(&mut self, new_hatype: u16) {
        self.hatype = new_hatype
    }

    pub fn set_pkttype(&mut self, new_pkttype: u8) {
        self.pkttype = new_pkttype
    }

    pub fn set_halen(&mut self, new_halen: u8) {
        self.halen = new_halen
    }

    pub fn set_addr(&mut self, new_addr: [u8; 8]) {
        self.addr = new_addr
    }
}

impl Default for LinkLayerSocketAddr {
    fn default() -> Self {
        Self::new(0, 0, 0, 0, 0, [0; 8])
    }
}
