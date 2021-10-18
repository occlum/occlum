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

    pub fn is_default(&self) -> bool {
        self.bits == Self::DEFAULT.bits
    }

    pub fn apply_perms(protect_range: &VMRange, perms: VMPerms) {
        extern "C" {
            pub fn occlum_ocall_mprotect(
                retval: *mut i32,
                addr: *const c_void,
                len: usize,
                prot: i32,
            ) -> sgx_status_t;
        };

        unsafe {
            let mut retval = 0;
            let addr = protect_range.start() as *const c_void;
            let len = protect_range.size();
            // PT_GROWSDOWN should only be applied to stack segment or a segment mapped with the MAP_GROWSDOWN flag set.
            // Since the memory are managed by our own, mprotect ocall shouldn't use this flag. Otherwise, EINVAL will be thrown.
            let mut prot = perms.clone();
            prot.remove(VMPerms::GROWSDOWN);
            let sgx_status = occlum_ocall_mprotect(&mut retval, addr, len, prot.bits() as i32);
            assert!(sgx_status == sgx_status_t::SGX_SUCCESS && retval == 0);
        }
    }
}

impl Default for VMPerms {
    fn default() -> Self {
        VMPerms::DEFAULT
    }
}
