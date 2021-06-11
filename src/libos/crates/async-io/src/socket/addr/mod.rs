use std::fmt::Debug;

use super::Domain;
use crate::prelude::*;

mod ipv4;
mod unix;

/// A trait for network addresses.
pub trait Addr: Clone + Debug + Send + Sync {
    /// Return the domain that the address belongs to.
    fn domain() -> Domain;

    /// Construct a new address from C's sockaddr_storage.
    ///
    /// The length argument specify how much bytes in the given sockaddr_storage are to be
    /// interpreted as parts of the address.
    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self>;

    /// Converts the address to C's sockaddr_storage.
    ///
    /// The actual length used in sockaddr_storage is also returned.
    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize);
}

pub use self::ipv4::{Ipv4Addr, Ipv4SocketAddr};
pub use self::unix::UnixAddr;
