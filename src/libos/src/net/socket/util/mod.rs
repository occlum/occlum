use crate::prelude::*;
use crate::untrusted::{
    SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSlice, UntrustedSliceAlloc,
};
use std;

mod domain;
mod flags;
mod iovs;
mod msg;
mod protocol;
mod shutdown;
mod socket_address;
mod r#type;

pub use self::domain::Domain;
pub use self::flags::{MsgFlags, RecvFlags, SendFlags};
pub use self::iovs::{Iovs, IovsMut, SliceAsLibcIovec};
pub use self::msg::{CMessages, CmsgData, MsgHdr, MsgHdrMut};
pub use self::protocol::SocketProtocol;
pub use self::r#type::Type;
pub use self::shutdown::Shutdown;
pub use self::socket_address::SockAddr;
