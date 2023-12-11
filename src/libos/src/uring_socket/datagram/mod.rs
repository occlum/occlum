//! Datagram sockets.
mod generic;
mod netlink; // Netlink is a datagram-oriented service and thus can reuse most of the code here
mod receiver;
mod sender;

use self::receiver::Receiver;
use self::sender::Sender;
use crate::prelude::*;
use crate::uring_socket::common::{do_bind, do_connect, Common};
use crate::uring_socket::ioctl::*;
use crate::uring_socket::runtime::Runtime;
use crate::uring_socket::sockopt::*;

pub use generic::DatagramSocket;
pub use netlink::NetlinkSocket;

use crate::uring_socket::socket::{
    timeout_to_timeval, GetRecvTimeoutCmd, GetSendTimeoutCmd, SetRecvTimeoutCmd, SetSendTimeoutCmd,
};

const MAX_BUF_SIZE: usize = 64 * 1024;
const OPTMEM_MAX: usize = 64 * 1024;
