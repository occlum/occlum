use crate::{net::AddressFamily, prelude::*};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use {errno, errno::Result};

/// A network domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
pub enum Domain {
    Ipv4 = libc::AF_INET,
    Ipv6 = libc::AF_INET6,
}

impl TryFrom<AddressFamily> for Domain {
    type Error = errno::Error;

    fn try_from(addr_family: AddressFamily) -> Result<Self> {
        match addr_family {
            AddressFamily::INET => Ok(Domain::Ipv4),
            AddressFamily::INET6 => Ok(Domain::Ipv6),
            _ => {
                return_errno!(Errno::EINVAL, "invalid uring domain");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn from_i32() {
        // Positive cases
        assert!(Domain::try_from(libc::AF_INET).unwrap() == Domain::Ipv4);
        assert!(Domain::try_from(libc::AF_INET6).unwrap() == Domain::Ipv6);

        // Negative cases
        assert!(Domain::try_from(-1).is_err());
    }
}
