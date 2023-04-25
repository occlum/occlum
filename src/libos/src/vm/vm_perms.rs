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

    pub fn can_lazy_extend(old_perms: VMPerms, new_perms: VMPerms) -> bool {
        debug_assert!(old_perms != new_perms);

        if old_perms > new_perms {
            return false;
        }

        if old_perms == VMPerms::NONE || old_perms == VMPerms::READ {
            return true;
        }
        if old_perms == VMPerms::READ | VMPerms::WRITE {
            return new_perms - old_perms >= VMPerms::EXEC;
        }
        if old_perms == VMPerms::READ | VMPerms::EXEC {
            return new_perms - old_perms >= VMPerms::WRITE;
        }

        // TODO: Maybe there is other rules, we can add them when we identify them.
        return false;
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
