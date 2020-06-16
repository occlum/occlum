use super::*;

bitflags! {
    pub struct VMPerms : u32 {
        const READ        = 0x1;
        const WRITE       = 0x2;
        const EXEC        = 0x4;
        const ALL         = Self::READ.bits | Self::WRITE.bits | Self::EXEC.bits;
    }
}

impl VMPerms {
    pub fn from_u32(bits: u32) -> Result<VMPerms> {
        Self::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid bits"))
    }

    pub fn can_read(&self) -> bool {
        self.contains(VMPerms::READ)
    }

    pub fn can_write(&self) -> bool {
        self.contains(VMPerms::WRITE)
    }

    pub fn can_execute(&self) -> bool {
        self.contains(VMPerms::EXEC)
    }
}

impl Default for VMPerms {
    fn default() -> Self {
        VMPerms::ALL
    }
}
