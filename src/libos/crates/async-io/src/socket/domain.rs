/// A network domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Ipv4,
    Ipv6,
    Unix,
}
