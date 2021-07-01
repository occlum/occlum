use super::*;

bitflags! {
   pub struct HowToShut: c_int {
        const READ = 0;
        const WRITE = 1;
        const BOTH = 2;
   }
}

impl HowToShut {
    pub fn try_from_raw(how: c_int) -> Result<Self> {
        match how {
            0 => Ok(Self::READ),
            1 => Ok(Self::WRITE),
            2 => Ok(Self::BOTH),
            _ => return_errno!(EINVAL, "invalid how"),
        }
    }

    pub fn to_shut_read(&self) -> bool {
        *self == Self::READ || *self == Self::BOTH
    }

    pub fn to_shut_write(&self) -> bool {
        *self == Self::WRITE || *self == Self::BOTH
    }
}
