use super::*;

use super::ipc::SHM_MANAGER;
use crate::ctor::dtor;
use crate::util::pku_util;
use config::LIBOS_CONFIG;
use vm_manager::VMManager;

use std::ops::{Deref, DerefMut};

const RSRV_MEM_PERM: MemPerm =
    MemPerm::from_bits_truncate(MemPerm::READ.bits() | MemPerm::WRITE.bits());

/// The virtual memory manager for the entire user space
pub struct UserSpaceVMManager(VMManager);

impl UserSpaceVMManager {
    fn new() -> Result<UserSpaceVMManager> {
        // TODO: Use reserved memory API for init space and use EDMM API for max space.
        let rsrv_mem_size = LIBOS_CONFIG.resource_limits.user_space_init_size;
        let vm_range = unsafe {
            // TODO: Current sgx_alloc_rsrv_mem implementation will commit all the pages of the desired size, which will consume
            // a lot of time. When EDMM is supported, there is no need to commit all the pages at the initialization stage. A function
            // which reserves memory but not commit pages should be provided then.
            let ptr = sgx_alloc_rsrv_mem(rsrv_mem_size);
            if ptr.is_null() {
                return_errno!(ENOMEM, "run out of reserved memory");
            }

            // Without EDMM support and the ReservedMemExecutable is set to 1, the reserved memory will be RWX. And we can't change the reserved memory permission.
            // With EDMM support, the reserved memory permission is RW by default. And we can change the permissions when needed.

            let addr = ptr as usize;
            debug!(
                "allocated rsrv addr is 0x{:x}, len is 0x{:x}",
                addr, rsrv_mem_size
            );
            pku_util::pkey_mprotect_userspace_mem(addr, rsrv_mem_size, RSRV_MEM_PERM.bits());
            VMRange::from_unchecked(addr, addr + rsrv_mem_size)
        };

        let vm_manager = VMManager::init(vm_range)?;

        Ok(UserSpaceVMManager(vm_manager))
    }

    pub fn get_total_size(&self) -> usize {
        self.range().size()
    }
}

// This will be called after all libos processes exit. Static variables are still safe to visit at this time.
pub async fn free_user_space() {
    SHM_MANAGER.clean_when_libos_exit().await;
    let range = USER_SPACE_VM_MANAGER.range();
    assert!(USER_SPACE_VM_MANAGER.verified_clean_when_exit().await);
    let addr = range.start();
    let size = range.size();
    info!("free user space VM: {:?}", range);
    pku_util::clear_pku_when_libos_exit(addr, size, RSRV_MEM_PERM.bits());
    assert!(unsafe { sgx_free_rsrv_mem(addr as *const c_void, size) == 0 });
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
}
