use std::fmt::Debug;

use super::Domain;

mod ipv4;
mod unix;

/// A trait for network addresses.
pub trait Addr: Debug {
    fn domain(&self) -> Domain;
}

pub use self::ipv4::{Ipv4Addr, Ipv4SocketAddr};
pub use self::unix::UnixAddr;
