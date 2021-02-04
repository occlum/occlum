use super::*;
use config::LIBOS_CONFIG;

/// The virtual memory manager for the entire user space
pub struct UserSpaceVMManager {
    total_size: usize,
    free_size: SgxMutex<usize>,
}

impl UserSpaceVMManager {
    fn new() -> UserSpaceVMManager {
        let rsrv_mem_size = LIBOS_CONFIG.resource_limits.user_space_size;
        UserSpaceVMManager {
            total_size: rsrv_mem_size,
            free_size: SgxMutex::new(rsrv_mem_size),
        }
    }

    pub fn alloc(&self, size: usize) -> Result<UserSpaceVMRange> {
        let vm_range = unsafe {
            let ptr = sgx_alloc_rsrv_mem(size);
            let perm = MemPerm::READ | MemPerm::WRITE;
            if ptr.is_null() {
                return_errno!(ENOMEM, "run out of reserved memory");
            }
            // Change the page permission to RW (default)
            assert!(sgx_tprotect_rsrv_mem(ptr, size, perm.bits()) == sgx_status_t::SGX_SUCCESS);

            let addr = ptr as usize;
            debug!("allocated rsrv addr is 0x{:x}, len is 0x{:x}", addr, size);
            VMRange::from_unchecked(addr, addr + size)
        };

        *self.free_size.lock().unwrap() -= size;
        Ok(UserSpaceVMRange::new(vm_range))
    }

    fn add_free_size(&self, user_space_vmrange: &UserSpaceVMRange) {
        *self.free_size.lock().unwrap() += user_space_vmrange.range().size();
    }

    // The empty range is not added to sub_range
    pub fn alloc_dummy(&self) -> UserSpaceVMRange {
        let empty_user_vm_range = unsafe { VMRange::from_unchecked(0, 0) };
        UserSpaceVMRange::new(empty_user_vm_range)
    }

    pub fn get_total_size(&self) -> usize {
        self.total_size
    }

    pub fn get_free_size(&self) -> usize {
        *self.free_size.lock().unwrap()
    }
}

lazy_static! {
    pub static ref USER_SPACE_VM_MANAGER: UserSpaceVMManager = UserSpaceVMManager::new();
}

bitflags! {
    struct MemPerm: i32 {
        const READ  = 1;
        const WRITE = 2;
        const EXEC  = 4;
    }
}

extern "C" {
    // Allocate a range of EPC memory from the reserved memory area with RW permission
    //
    // Parameters:
    // Inputs: length [in]: Size of region to be allocated in bytes. Page aligned
    // Return: Starting address of the new allocated memory area on success; otherwise NULL
    //
    fn sgx_alloc_rsrv_mem(length: usize) -> *const c_void;

    // Free a range of EPC memory from the reserved memory area
    //
    // Parameters:
    // Inputs: addr[in]: Starting address of region to be freed. Page aligned.
    //         length[in]: The length of the memory to be freed in bytes.  Page aligned
    // Return: 0 on success; otherwise -1
    //
    fn sgx_free_rsrv_mem(addr: *const c_void, length: usize) -> i32;

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
}

#[derive(Debug)]
pub struct UserSpaceVMRange {
    vm_range: VMRange,
}

impl UserSpaceVMRange {
    fn new(vm_range: VMRange) -> UserSpaceVMRange {
        UserSpaceVMRange { vm_range }
    }

    pub fn range(&self) -> &VMRange {
        &self.vm_range
    }
}

impl Drop for UserSpaceVMRange {
    fn drop(&mut self) {
        let addr = self.vm_range.start() as *const c_void;
        let size = self.vm_range.size();
        if size == 0 {
            return;
        }

        USER_SPACE_VM_MANAGER.add_free_size(self);

        assert!(unsafe { sgx_free_rsrv_mem(addr, size) == 0 });
    }
}
