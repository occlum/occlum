use crate::prelude::*;
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// A network type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
pub enum SocketType {
    STREAM = libc::SOCK_STREAM,
    DGRAM = libc::SOCK_DGRAM,
    RAW = libc::SOCK_RAW,
    RDM = libc::SOCK_RDM,
    SEQPACKET = libc::SOCK_SEQPACKET,
    DCCP = libc::SOCK_DCCP,
    PACKET = libc::SOCK_PACKET,
}
