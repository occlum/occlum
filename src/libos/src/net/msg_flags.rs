use super::*;

// TODO: use bitflag! to make this memory safe
#[derive(Debug, Copy, Clone, Default)]
pub struct MsgFlags {
    bits: u32,
}

impl MsgFlags {
    pub fn from_u32(c_flags: u32) -> Result<MsgFlags> {
        Ok(MsgFlags { bits: 0 })
    }

    pub fn to_u32(&self) -> u32 {
        self.bits
    }
}
