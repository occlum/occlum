use super::*;

use crate::config::LIBOS_CONFIG;
use crate::ctor::dtor;
use crate::ipc::SYSTEM_V_SHM_MANAGER;
use crate::util::pku_util;

use std::ops::{Deref, DerefMut};
use vm_epc::SGXPlatform;
use vm_manager::VMManager;
use vm_perms::VMPerms;

const USER_SPACE_DEFAULT_MEM_PERM: VMPerms = VMPerms::DEFAULT;

/// The virtual memory manager for the entire user space
pub struct UserSpaceVMManager {
    inner: VMManager,
    sgx_platform: SGXPlatform,
}

impl UserSpaceVMManager {
    fn new() -> Result<UserSpaceVMManager> {
        let sgx_platform = SGXPlatform::new();
        let init_size = LIBOS_CONFIG.resource_limits.user_space_init_size;
        let max_size = LIBOS_CONFIG.resource_limits.user_space_max_size;
        info!(
            "alloc user space init size = {:?}, max size = {:?}",
            init_size, max_size
        );

        let (userspace_vm_range, gap_range) = sgx_platform.alloc_user_space(init_size, max_size)?;

        // Use pkey_mprotect to set the whole userspace to R/W permissions. If user specifies a new
        // permission, the mprotect ocall will update the permission.
        pku_util::pkey_mprotect_userspace_mem(
            &userspace_vm_range,
            gap_range.as_ref(),
            USER_SPACE_DEFAULT_MEM_PERM,
        );

        let vm_manager = VMManager::init(userspace_vm_range, gap_range)?;

        info!(
            "user space allocated, total size = {:?}",
            userspace_vm_range.size()
        );

        Ok(Self {
            inner: vm_manager,
            sgx_platform,
        })
    }

    pub fn get_total_size(&self) -> usize {
        self.range().size()
    }

    pub fn get_user_space_ranges(&self) -> [VMRange; 2] {
        let total_user_space_range = self.range();
        let gap_range = self.gap_range();
        if let Some(gap) = gap_range {
            // There are two parts of user space
            let (part_a_start_addr, part_b_end_addr) =
                (total_user_space_range.start(), total_user_space_range.end());
            let (part_a_end_addr, part_b_start_addr) = (gap.start(), gap.end());
            let user_space_range_a = VMRange::new(part_a_start_addr, part_a_end_addr).unwrap();
            let user_space_range_b = VMRange::new(part_b_start_addr, part_b_end_addr).unwrap();
            [user_space_range_a, user_space_range_b]
        } else {
            // There is no gap. Thus set the part B memory range to zero
            let (part_a_start_addr, part_a_end_addr) =
                (total_user_space_range.start(), total_user_space_range.end());
            let user_space_range_a = VMRange::new(part_a_start_addr, part_a_end_addr).unwrap();
            let user_space_range_b = unsafe { VMRange::from_unchecked(0, 0) };
            [user_space_range_a, user_space_range_b]
        }
    }
}

// This provides module teardown function attribute similar with `__attribute__((destructor))` in C/C++ and will
// be called after the main function. Static variables are still safe to visit at this time.
#[dtor]
fn free_user_space() {
    info!("free user space at the end");
    SYSTEM_V_SHM_MANAGER.clean_when_libos_exit();
    let total_user_space_range = USER_SPACE_VM_MANAGER.range();
    let gap_range = USER_SPACE_VM_MANAGER.gap_range();
    assert!(USER_SPACE_VM_MANAGER.verified_clean_when_exit());
    let addr = total_user_space_range.start();
    let size = total_user_space_range.size();
    debug!("free user space VM: {:?}", total_user_space_range);

    pku_util::clear_pku_when_libos_exit(
        total_user_space_range,
        gap_range.as_ref(),
        USER_SPACE_DEFAULT_MEM_PERM,
    );

    USER_SPACE_VM_MANAGER
        .sgx_platform
        .free_user_space(total_user_space_range, gap_range.as_ref());
}

impl Deref for UserSpaceVMManager {
    type Target = VMManager;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

lazy_static! {
    pub static ref USER_SPACE_VM_MANAGER: UserSpaceVMManager = UserSpaceVMManager::new().unwrap();
}
