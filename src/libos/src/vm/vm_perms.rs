use super::*;

bitflags! {
    pub struct VMPerms : u32 {
        const NONE        = 0x0;
        const READ        = 0x1;
        const WRITE       = 0x2;
        const EXEC        = 0x4;
        const DEFAULT     = Self::READ.bits | Self::WRITE.bits;
        const ALL         = Self::DEFAULT.bits | Self::EXEC.bits;
        const GROWSDOWN   = 0x01000000; // For x86, stack direction always grow downwards.
    }
}

impl VMPerms {
    pub fn from_u32(bits: u32) -> Result<VMPerms> {
        let mut perms = Self::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid bits"))?;

        // SGX SDK doesn't accept permissions like write or exec without read.
        if perms != VMPerms::NONE {
            perms |= VMPerms::READ
        }
        Ok(perms)
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

    pub fn is_default(&self) -> bool {
        self.bits == Self::DEFAULT.bits
    }

    pub fn display(&self) -> String {
        let mut str = String::new();
        if self.can_read() {
            str += "r";
        } else {
            str += "-";
        }
        if self.can_write() {
            str += "w";
        } else {
            str += "-";
        }
        if self.can_execute() {
            str += "x";
        } else {
            str += "-";
        }
        str
    }
}

impl Default for VMPerms {
    fn default() -> Self {
        VMPerms::DEFAULT
    }
}
