use crate::prelude::*;
use crate::untrusted::{
    SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSlice, UntrustedSliceAlloc,
};
use std;

mod addr;
mod any_addr;
mod domain;
mod flags;
mod iovs;
mod msg;
mod protocol;
mod shutdown;
mod r#type;

pub use self::addr::{
    Addr, CSockAddr, Ipv4Addr, Ipv4SocketAddr, Ipv6SocketAddr, SockAddr, UnixAddr,
};
pub use self::any_addr::AnyAddr;
pub use self::domain::Domain;
pub use self::flags::{mmsghdr, MsgFlags, RecvFlags, SendFlags, SocketFlags};
pub use self::iovs::{Iovs, IovsMut, SliceAsLibcIovec};
pub use self::msg::{CMessages, CmsgData};
pub use self::protocol::SocketProtocol;
pub use self::r#type::SocketType;
pub use self::shutdown::Shutdown;
