use std::any::Any;
use std::fmt::Debug;

use super::Domain;
use crate::prelude::*;

mod c_sock_addr;
mod ipv4;
mod ipv6;
mod unix;

/// A trait for network addresses.
pub trait Addr: Clone + Debug + Default + PartialEq + Send + Sync {
    /// Return the domain that the address belongs to.
    fn domain() -> Domain
    where
        Self: Sized;

    /// Construct a new address from C's sockaddr_storage.
    ///
    /// The length argument specify how much bytes in the given sockaddr_storage are to be
    /// interpreted as parts of the address.
    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self>
    where
        Self: Sized;

    /// Converts the address to C's sockaddr_storage.
    ///
    /// The actual length used in sockaddr_storage is also returned.
    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize);

    fn as_any(&self) -> &dyn Any;
}

pub use self::c_sock_addr::CSockAddr;
pub use self::ipv4::{Ipv4Addr, Ipv4SocketAddr};
pub use self::ipv6::{Ipv6Addr, Ipv6SocketAddr};
pub use self::unix::UnixAddr;

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;

    #[test]
    fn ipv4_to_and_from_c() {
        let addr = [127u8, 0, 0, 1];
        let port = 8888u16;

        let c_addr = libc::sockaddr_in {
            sin_family: libc::AF_INET as _,
            sin_port: port.to_be(),
            sin_addr: libc::in_addr {
                s_addr: u32::from_be_bytes(addr).to_be(),
            },
            sin_zero: [0u8; 8],
        };

        let addr = {
            let addr = Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
            Ipv4SocketAddr::new(addr, port)
        };

        check_to_and_from_c(&c_addr, &addr);
    }

    #[test]
    fn unix_unnamed_to_and_from_c() {
        let c_addr = libc::sockaddr_un {
            sun_family: libc::AF_UNIX as _,
            sun_path: [0; 108],
        };
        let c_addr_len = size_of::<libc::sa_family_t>() + 0;

        let addr = UnixAddr::Unnamed;

        check_to_and_from_c(&(c_addr, c_addr_len), &addr);
    }

    #[test]
    fn unix_pathname_to_and_from_c() {
        let path = "test.sock";
        let c_addr = {
            let mut c_addr = libc::sockaddr_un {
                sun_family: libc::AF_UNIX as _,
                sun_path: [0; 108],
            };
            let sun_path: &mut [u8] = unsafe {
                let path_len = path.as_bytes().len();
                // Pathname must be a null-terminated C string
                c_addr.sun_path[path_len] = 0;
                std::mem::transmute(&mut c_addr.sun_path[..path_len])
            };
            sun_path.copy_from_slice(path.as_bytes());
            c_addr
        };
        let c_addr_len = size_of::<libc::sa_family_t>() + path.as_bytes().len() + 1;

        let addr = UnixAddr::Pathname(path.to_string());

        check_to_and_from_c(&(c_addr, c_addr_len), &addr);
    }

    #[test]
    fn unix_abstract_to_and_from_c() {
        let name = "an\nabstract\tname";
        let c_addr = {
            let mut c_addr = libc::sockaddr_un {
                sun_family: libc::AF_UNIX as _,
                sun_path: [0; 108],
            };
            let sun_path: &mut [u8] = unsafe {
                let name_len = name.as_bytes().len();
                // Abstract name must have a path starting with a null byte.
                c_addr.sun_path[0] = 0;
                std::mem::transmute(&mut c_addr.sun_path[1..name_len + 1])
            };
            sun_path.copy_from_slice(name.as_bytes());
            c_addr
        };
        let c_addr_len = size_of::<libc::sa_family_t>() + name.as_bytes().len() + 1;

        let addr = UnixAddr::Abstract(name.as_bytes().to_vec());

        check_to_and_from_c(&(c_addr, c_addr_len), &addr);
    }

    fn check_to_and_from_c<T: CSockAddr, U: Addr>(c_addr: &T, addr: &U) {
        let c_addr_storage = c_addr.to_c_storage();

        // To C
        assert!(are_sock_addrs_equal(c_addr, &addr.to_c_storage()));
        assert!(are_sock_addrs_equal(&c_addr_storage, &addr.to_c_storage()));

        // From C
        let (c_addr_storage, c_addr_len) = c_addr_storage;
        assert!(&U::from_c_storage(&c_addr_storage, c_addr_len).unwrap() == addr);
    }

    fn are_sock_addrs_equal<T: CSockAddr, U: CSockAddr>(one: &T, other: &U) -> bool {
        one.c_family() == other.c_family() && one.c_addr() == other.c_addr()
    }
}
