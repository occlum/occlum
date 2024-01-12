use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Shutdown {
    Read = 0,
    Write = 1,
    Both = 2,
}

impl Shutdown {
    pub fn from_c(c_val: u32) -> Result<Self> {
        match c_val {
            0 => Ok(Self::Read),
            1 => Ok(Self::Write),
            2 => Ok(Self::Both),
            _ => return_errno!(EINVAL, "invalid how"),
        }
    }

    pub fn to_c(&self) -> u32 {
        *self as u32
    }

    pub fn should_shut_read(&self) -> bool {
        // a slightly more efficient check than using two equality comparions
        self.to_c() % 2 == 0
    }

    pub fn should_shut_write(&self) -> bool {
        // a slightly more efficient check than using two equality comparions
        self.to_c() >= 1
    }
}
