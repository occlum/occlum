use super::ipc::SHM_MANAGER;
use super::*;
use crate::ctor::dtor;
use crate::util::pku_util;
use config::LIBOS_CONFIG;
use std::ops::{Deref, DerefMut};
use vm_epc::SGXPlatform;
use vm_manager::VMManager;
use vm_perms::VMPerms;

const RSRV_MEM_PERM: VMPerms = VMPerms::DEFAULT;

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

        let (userspace_vm_range, gap_range) = sgx_platform.alloc_user_space(init_size, max_size)?;

        info!(
            "user space allocated, range = {:?}, gap_range = {:?}",
            userspace_vm_range, gap_range
        );

        // FIXME
        // pku_util::pkey_mprotect_userspace_mem(addr, user_space_mem_size, RSRV_MEM_PERM.bits());

        let vm_manager = VMManager::init_with_mem_gap(userspace_vm_range, gap_range)?;

        Ok(Self {
            inner: vm_manager,
            sgx_platform,
        })
    }

    pub fn get_total_size(&self) -> usize {
        self.range().size()
    }
}

pub fn free_user_space() {
    info!("free user space at the end");
    SHM_MANAGER.clean_when_libos_exit();
    let total_user_space_range = USER_SPACE_VM_MANAGER.range();
    assert!(USER_SPACE_VM_MANAGER.verified_clean_when_exit());
    let addr = total_user_space_range.start();
    let size = total_user_space_range.size();
    info!("free user space VM: {:?}", total_user_space_range);

    // FIXME
    // pku_util::clear_pku_when_libos_exit(addr, size, RSRV_MEM_PERM.bits());

    let gap_range = USER_SPACE_VM_MANAGER
        .gap_range()
        .expect("Gap range must exists");
    USER_SPACE_VM_MANAGER
        .sgx_platform
        .free_user_space(total_user_space_range, &gap_range);
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
