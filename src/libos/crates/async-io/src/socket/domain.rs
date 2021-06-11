/// A network domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Domain {
    Ipv4 = libc::AF_INET,
    Ipv6 = libc::AF_INET6,
    Unix = libc::AF_LOCAL,
}
