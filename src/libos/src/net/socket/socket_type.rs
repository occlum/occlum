use super::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum SocketType {
    STREAM = 1,
    DGRAM = 2,
    RAW = 3,
    RDM = 4,
    SEQPACKET = 5,
    DCCP = 6,
    PACKET = 10,
}

impl SocketType {
    pub fn try_from(sock_type: i32) -> Result<Self> {
        match sock_type {
            1 => Ok(SocketType::STREAM),
            2 => Ok(SocketType::DGRAM),
            3 => Ok(SocketType::RAW),
            4 => Ok(SocketType::RDM),
            5 => Ok(SocketType::SEQPACKET),
            6 => Ok(SocketType::DCCP),
            10 => Ok(SocketType::PACKET),
            _ => return_errno!(EINVAL, "invalid socket type"),
        }
    }
}
