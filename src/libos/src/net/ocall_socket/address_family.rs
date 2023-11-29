use super::*;

// The protocol family generally is the same as the address family
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum AddressFamily {
    UNSPEC = 0,
    LOCAL = 1,
    /* Hide the families with the same number
    UNIX       = LOCAL,
    FILE       = LOCAL,
    */
    INET = 2,
    AX25 = 3,
    IPX = 4,
    APPLETALK = 5,
    NETROM = 6,
    BRIDGE = 7,
    ATMPVC = 8,
    X25 = 9,
    INET6 = 10,
    ROSE = 11,
    DECnet = 12,
    NETBEUI = 13,
    SECURITY = 14,
    KEY = 15,
    NETLINK = 16,
    /* Hide the family with the same number
    ROUTE      = NETLINK,
    */
    PACKET = 17,
    ASH = 18,
    ECONET = 19,
    ATMSVC = 20,
    RDS = 21,
    SNA = 22,
    IRDA = 23,
    PPPOX = 24,
    WANPIPE = 25,
    LLC = 26,
    IB = 27,
    MPLS = 28,
    CAN = 29,
    TIPC = 30,
    BLUETOOTH = 31,
    IUCV = 32,
    RXRPC = 33,
    ISDN = 34,
    PHONET = 35,
    IEEE802154 = 36,
    CAIF = 37,
    ALG = 38,
    NFC = 39,
    VSOCK = 40,
    KCM = 41,
    QIPCRTR = 42,
    SMC = 43,
    XDP = 44,
    MAX = 45,
}

impl AddressFamily {
    pub fn try_from(af: u16) -> Result<Self> {
        if af >= Self::MAX as u16 {
            return_errno!(EINVAL, "Unknown address family");
        } else {
            Ok(unsafe { core::mem::transmute(af) })
        }
    }
}
