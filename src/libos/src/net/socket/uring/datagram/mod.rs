//! Datagram sockets.
mod generic;
mod netlink; // Netlink is a datagram-oriented service and thus can reuse most of the code here
mod receiver;
mod sender;

use self::receiver::Receiver;
use self::sender::Sender;
use crate::net::socket::uring::common::{do_bind, do_connect, Common};
use crate::net::socket::uring::ioctl::*;
use crate::net::socket::uring::runtime::Runtime;
use crate::net::socket::uring::sockopt::*;
use crate::prelude::*;

pub use generic::DatagramSocket;
pub use netlink::NetlinkSocket;

use crate::net::socket::uring::misc::{
    timeout_to_timeval, GetRecvTimeoutCmd, GetSendTimeoutCmd, SetRecvTimeoutCmd, SetSendTimeoutCmd,
};

const MAX_BUF_SIZE: usize = 64 * 1024;
const OPTMEM_MAX: usize = 64 * 1024;
