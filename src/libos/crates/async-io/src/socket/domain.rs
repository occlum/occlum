use num_enum::{IntoPrimitive, TryFromPrimitive};

/// A network domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
pub enum Domain {
    Ipv4 = libc::AF_INET,
    Unix = libc::AF_LOCAL,
    Ipv6 = libc::AF_INET6,
    Netlink = libc::AF_NETLINK,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn from_i32() {
        // Positive cases
        assert!(Domain::try_from(libc::AF_INET).unwrap() == Domain::Ipv4);
        assert!(Domain::try_from(libc::AF_LOCAL).unwrap() == Domain::Unix);
        assert!(Domain::try_from(libc::AF_INET6).unwrap() == Domain::Ipv6);
        assert!(Domain::try_from(libc::AF_NETLINK).unwrap() == Domain::Netlink);

        // Negative cases
        assert!(Domain::try_from(-1).is_err());
    }
}
