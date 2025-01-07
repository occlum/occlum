//! Datagram sockets.
mod generic;
mod ip_packet;
mod netlink;
mod raw_packet;
mod receiver;
mod sender;

use self::receiver::Receiver;
use self::sender::Sender;
use crate::net::socket::sockopt::*;
use crate::net::socket::uring::common::{do_bind, do_connect, Common};
use crate::net::socket::uring::runtime::Runtime;
use crate::prelude::*;

pub use generic::DatagramSocket;
pub use ip_packet::IpPacket;
pub use netlink::NetlinkSocket;
pub use raw_packet::RawPacket;

use crate::net::socket::sockopt::{
    timeout_to_timeval, GetRecvTimeoutCmd, GetSendTimeoutCmd, SetRecvTimeoutCmd, SetSendTimeoutCmd,
};

const MAX_BUF_SIZE: usize = 64 * 1024;
const OPTMEM_MAX: usize = 64 * 1024;
const IPV4HDR_SIZE: usize = 20;
const IPV6HDR_SIZE: usize = 40;
