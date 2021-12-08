use super::*;
use std::alloc::{AllocError, Allocator, Layout};
use std::ptr::{self, write_bytes, NonNull};

/// The global memory allocator for untrusted memory
pub static mut UNTRUSTED_ALLOC: UntrustedAlloc = UntrustedAlloc;

pub struct UntrustedAlloc;

unsafe impl Allocator for UntrustedAlloc {
    fn allocate(&self, layout: Layout) -> std::result::Result<NonNull<[u8]>, AllocError> {
        if layout.size() == 0 {
            return Err(AllocError);
        }

        // Do OCall to allocate the untrusted memory according to the given layout
        let layout = layout
            .align_to(std::mem::size_of::<*const c_void>())
            .unwrap();
        let mem_ptr = {
            let mut mem_ptr: *mut c_void = ptr::null_mut();
            let sgx_status = unsafe {
                occlum_ocall_posix_memalign(&mut mem_ptr as *mut _, layout.align(), layout.size())
            };
            debug_assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
            mem_ptr
        } as *mut u8;
        if mem_ptr == std::ptr::null_mut() {
            return Err(AllocError);
        }

        // Sanity checks
        // Post-condition 1: alignment
        debug_assert!(mem_ptr as usize % layout.align() == 0);
        // Post-condition 2: out-of-enclave
        assert!(sgx_trts::trts::rsgx_raw_is_outside_enclave(
            mem_ptr as *const u8,
            layout.size()
        ));
        Ok(NonNull::new(unsafe {
            core::slice::from_raw_parts_mut(mem_ptr, layout.size() as usize)
        })
        .unwrap())
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Pre-condition: out-of-enclave
        debug_assert!(sgx_trts::trts::rsgx_raw_is_outside_enclave(
            ptr.as_ptr(),
            layout.size()
        ));

        let sgx_status = unsafe { occlum_ocall_free(ptr.as_ptr() as *mut c_void) };
        debug_assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
    }
}

extern "C" {
    fn occlum_ocall_posix_memalign(
        ptr: *mut *mut c_void,
        align: usize, // must be power of two and a multiple of sizeof(void*)
        size: usize,
    ) -> sgx_status_t;
    fn occlum_ocall_free(ptr: *mut c_void) -> sgx_status_t;
}
