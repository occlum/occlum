use super::*;

use std::sync::atomic::{AtomicBool, Ordering};

/// Status variable accessed by assembly code
#[no_mangle]
pub static mut pku_enabled: u64 = 0;

lazy_static! {
    pub static ref PKU_ENABLED: AtomicBool = AtomicBool::new(false);
}

const PKEY_LIBOS: i32 = 0;
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
    PKU_ENABLED.store(true, Ordering::Release);
}

pub fn check_pku_enabled() -> bool {
    PKU_ENABLED.load(Ordering::Acquire)
}

pub fn pkey_mprotect_userspace_mem(user_mem_base: usize, user_mem_len: usize, perm: i32) {
    if !self::check_pku_enabled() {
        return;
    }
    let mut retval = -1;
    debug!(
        "associate memory region: 0x{:x} -> 0x{:x}, size: 0x{:x} with pkey for userspace: {:?}",
        user_mem_base,
        user_mem_base + user_mem_len,
        user_mem_len,
        PKEY_USER
    );
    let sgx_status = unsafe {
        occlum_ocall_pkey_mprotect(
            &mut retval,
            user_mem_base as *const c_void,
            user_mem_len,
            perm,
            PKEY_USER,
        )
    };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS && retval == 0);
}

pub fn clear_pku_when_libos_exit(user_mem_base: usize, user_mem_len: usize, perm: i32) {
    if !self::check_pku_enabled() {
        return;
    }
    let mut retval = -1;
    debug!(
        "re-associate memory region  0x{:x} -> 0x{:x}, size: 0x{:x} with pkey for libos: {:?}",
        user_mem_base,
        user_mem_base + user_mem_len,
        user_mem_len,
        PKEY_LIBOS
    );
    let sgx_status = unsafe {
        occlum_ocall_pkey_mprotect(
            &mut retval,
            user_mem_base as *const c_void,
            user_mem_len,
            perm,
            PKEY_LIBOS,
        )
    };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS && retval == 0);
    debug!("free pkey: {:?}", PKEY_USER);
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
