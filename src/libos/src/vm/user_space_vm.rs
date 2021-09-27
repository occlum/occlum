use super::*;
use crate::ctor::dtor;
use config::LIBOS_CONFIG;
use std::ops::{Deref, DerefMut};
use vm_manager::VMManager;

/// The virtual memory manager for the entire user space
pub struct UserSpaceVMManager(VMManager);

impl UserSpaceVMManager {
    fn new() -> Result<UserSpaceVMManager> {
        let rsrv_mem_size = LIBOS_CONFIG.resource_limits.user_space_size;
        let vm_range = unsafe {
            // TODO: Current sgx_alloc_rsrv_mem implmentation will commit all the pages of the desired size, which will consume
            // a lot of time. When EDMM is supported, there is no need to commit all the pages at the initialization stage. A function
            // which reserves memory but not commit pages should be provided then.
            let ptr = sgx_alloc_rsrv_mem(rsrv_mem_size);
            let perm = MemPerm::READ | MemPerm::WRITE;
            if ptr.is_null() {
                return_errno!(ENOMEM, "run out of reserved memory");
            }
            // Change the page permission to RW (default)
            assert!(
                sgx_tprotect_rsrv_mem(ptr, rsrv_mem_size, perm.bits()) == sgx_status_t::SGX_SUCCESS
            );

            let addr = ptr as usize;
            debug!(
                "allocated rsrv addr is 0x{:x}, len is 0x{:x}",
                addr, rsrv_mem_size
            );
            VMRange::from_unchecked(addr, addr + rsrv_mem_size)
        };

        let vm_manager = VMManager::init(vm_range)?;

        Ok(UserSpaceVMManager(vm_manager))
    }

    pub fn get_total_size(&self) -> usize {
        self.range().size()
    }
}

// This provides module teardown function attribute similar with `__attribute__((destructor))` in C/C++ and will
// be called after the main function. Static variables are still safe to visit at this time.
#[dtor]
fn free_user_space() {
    let range = USER_SPACE_VM_MANAGER.range();
    assert!(USER_SPACE_VM_MANAGER.verified_clean_when_exit());
    let addr = range.start() as *const c_void;
    let size = range.size();
    info!("free user space VM: {:?}", range);
    assert!(unsafe { sgx_free_rsrv_mem(addr, size) == 0 });
}

impl Deref for UserSpaceVMManager {
    type Target = VMManager;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

lazy_static! {
    pub static ref USER_SPACE_VM_MANAGER: UserSpaceVMManager = UserSpaceVMManager::new().unwrap();
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
