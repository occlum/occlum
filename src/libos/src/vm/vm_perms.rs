use super::*;

bitflags! {
    pub struct VMPerms : u32 {
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
        use sgx_trts::enclave::rsgx_is_supported_EDMM;

        unsafe {
            let mut retval = 0;
            let addr = protect_range.start() as *const c_void;
            let len = protect_range.size();
            // PT_GROWSDOWN should only be applied to stack segment or a segment mapped with the MAP_GROWSDOWN flag set.
            // Since the memory are managed by our own, mprotect ocall shouldn't use this flag. Otherwise, EINVAL will be thrown.
            let mut prot = perms.clone();
            prot.remove(VMPerms::GROWSDOWN);

            if rsgx_is_supported_EDMM() {
                // With EDMM support, reserved memory permission should be updated.
                // For sgx_tprotect_rsrv_mem, if the permission is only WRITE or EXEC, SGX_ERROR_INVALID_PARAMETER will return.
                // Thus, we add READ here.
                if !prot.can_read() && (prot.can_write() || prot.can_execute()) {
                    prot.insert(VMPerms::READ);
                }
                let sgx_status = sgx_tprotect_rsrv_mem(addr, len, prot.bits() as i32);
                if sgx_status != sgx_status_t::SGX_SUCCESS {
                    panic!("sgx_tprotect_rsrv_mem status {}", sgx_status);
                }
            } else {
                // Without EDMM support, reserved memory permission is statically RWX and we only need to do mprotect ocall.
                let sgx_status = occlum_ocall_mprotect(&mut retval, addr, len, prot.bits() as i32);
                if sgx_status != sgx_status_t::SGX_SUCCESS || retval != 0 {
                    panic!(
                        "occlum_ocall_mprotect status {}, retval {}",
                        sgx_status, retval
                    );
                }
            }
        }
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

extern "C" {
    // Modify the access permissions of the pages in the reserved memory area
    //
    // Parameters:
    // Inputs: addr[in]: Starting address of region which needs to change access
    //         permission. Page aligned.
    //         length[in]: The length of the memory to be manipulated in bytes. Page aligned.
    //         prot[in]: The target memory protection.
    // Return: sgx_status_t
    //
    fn sgx_tprotect_rsrv_mem(addr: *const c_void, length: usize, prot: i32) -> sgx_status_t;

    fn occlum_ocall_mprotect(
        retval: *mut i32,
        addr: *const c_void,
        len: usize,
        prot: i32,
    ) -> sgx_status_t;
}
