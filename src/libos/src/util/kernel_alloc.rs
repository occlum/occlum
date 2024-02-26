use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

// This file provides "KernelAlloc", a wrapper for sgx_std "System" allocator, is used as
// the global allocator for Occlum kernel. Currently, this can provides the ability to
// monitor the kernel heap usage.

pub struct KernelAlloc {
    size: AtomicUsize,
}

impl KernelAlloc {
    pub fn get_kernel_mem_size() -> Option<usize> {
        cfg_if! {
            if #[cfg(feature = "kernel_heap_monitor")] {
                Some(ALLOC.size.load(Ordering::Relaxed))
            } else {
                None
            }
        }
    }

    pub fn get_kernel_heap_config() -> usize {
        std::enclave::get_heap_size()
    }

    pub fn get_kernel_heap_peak_used() -> usize {
        sgx_trts::enclave::rsgx_get_peak_heap_used()
    }
}

unsafe impl GlobalAlloc for KernelAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            self.size.fetch_add(layout.size(), Ordering::Relaxed);
        }

        ptr
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            self.size.fetch_add(layout.size(), Ordering::Relaxed);
        }

        ptr
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        self.size.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = System.realloc(ptr, layout, new_size);
        if !ptr.is_null() {
            let old_size = layout.size();
            if new_size > old_size {
                // grow
                self.size.fetch_add(new_size - old_size, Ordering::Relaxed);
            } else if new_size < old_size {
                // shrink
                self.size.fetch_sub(old_size - new_size, Ordering::Relaxed);
            }
        }

        ptr
    }
}

#[cfg(feature = "kernel_heap_monitor")]
#[global_allocator]
static ALLOC: KernelAlloc = KernelAlloc {
    size: AtomicUsize::new(0),
};
