//! Datagram sockets.
mod generic;
mod receiver;
mod sender;

use self::receiver::Receiver;
use self::sender::Sender;
use crate::net::socket::sockopt::*;
use crate::net::socket::uring::common::{do_bind, do_connect, Common};
use crate::net::socket::uring::runtime::Runtime;
use crate::prelude::*;

pub use generic::DatagramSocket;

use crate::net::socket::sockopt::{
    timeout_to_timeval, GetRecvTimeoutCmd, GetSendTimeoutCmd, SetRecvTimeoutCmd, SetSendTimeoutCmd,
};

const MAX_BUF_SIZE: usize = 64 * 1024;
const OPTMEM_MAX: usize = 64 * 1024;
