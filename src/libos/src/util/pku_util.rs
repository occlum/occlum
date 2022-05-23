use super::*;

use std::sync::atomic::{AtomicBool, Ordering};

/// Status variable accessed by assembly code
#[no_mangle]
pub static mut pku_enabled: u64 = 0;

lazy_static! {
    pub static ref PKU_ENABLED: AtomicBool = AtomicBool::new(false);
}

const PKEY_USER: i32 = 1;

/// Try enable PKU features in Occlum.
pub fn try_set_pku_enabled() {
    // Alloc pkey
    let mut pkey = -1;
    let sgx_status = unsafe { occlum_ocall_pkey_alloc(&mut pkey, 0, 0) };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS && pkey == PKEY_USER);

    unsafe {
        pku_enabled = 1;
    }
    assert!(PKU_ENABLED.load(Ordering::Relaxed) == false);
    debug!("pku has been enabled");
    PKU_ENABLED.store(true, Ordering::Release);
}

pub fn check_pku_enabled() -> bool {
    PKU_ENABLED.load(Ordering::Acquire)
}

pub fn config_userspace_mem(baseaddr: usize, len: usize, perm: i32) {
    if !self::check_pku_enabled() {
        return;
    }
    debug!(
        "associate memory region: 0x{:x} -> 0x{:x}, size: 0x{:x} with pkey: {:?}",
        baseaddr,
        baseaddr + len,
        len,
        PKEY_USER
    );
    let mut retval = -1;
    let sgx_status = unsafe {
        occlum_ocall_pkey_mprotect(&mut retval, baseaddr as *const c_void, len, perm, PKEY_USER)
    };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS && retval == 0);
}

pub fn free_pkey_when_libos_exit() {
    if !self::check_pku_enabled() {
        return;
    }
    debug!("free pkey: {:?}", PKEY_USER);
    let mut retval = -1;
    let sgx_status = unsafe { occlum_ocall_pkey_free(&mut retval, PKEY_USER) };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS && retval == 0);
}

extern "C" {
    pub fn occlum_ocall_pkey_alloc(
        retval: *mut i32,
        flags: u32,
        access_rights: u32,
    ) -> sgx_status_t;

    pub fn occlum_ocall_pkey_mprotect(
        retval: *mut i32,
        addr: *const c_void,
        len: usize,
        prot: i32,
        pkey: i32,
    ) -> sgx_status_t;

    pub fn occlum_ocall_pkey_free(retval: *mut i32, pkey: i32) -> sgx_status_t;
}
