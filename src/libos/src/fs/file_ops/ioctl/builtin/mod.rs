//! Built-in ioctls.

use super::*;

pub use self::termios::*;
pub use self::winsize::*;
pub use host_socket::ioctl::{
    GetIfConf, GetIfReqWithRawCmd, GetReadBufLen, IfConf, IfReq, SetNonBlocking,
};

mod termios;
mod winsize;
