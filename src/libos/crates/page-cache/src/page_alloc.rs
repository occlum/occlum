use crate::page::PAGE_SIZE;

use std::alloc::{alloc, dealloc, Layout};
use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A page allocator that can allocate/deallocate pages from free memory
/// and monitor the amount of free memory.
pub trait PageAlloc: Send + Sync + Clone + 'static {
    /// Allocate a new page.
    fn alloc_page() -> *mut u8;

    /// Deallocate a page.
    /// The `page_ptr` must be a valid pointer obtained from `alloc_page`.
    unsafe fn dealloc_page(page_ptr: *mut u8);

    /// Triggered when memory is low.
    fn register_low_memory_callback(f: impl Fn());

    /// Whether the memory is consumed out.
    /// User can define own memory limit.
    fn is_memory_low() -> bool;
}

/// A test-purpose page allocator with fixed total size.
pub struct FixedSizePageAlloc {
    total_bytes: AtomicUsize,
    remain_bytes: AtomicUsize,
}

impl FixedSizePageAlloc {
    pub fn new(total_bytes: usize) -> Self {
        let new_self = Self {
            total_bytes: AtomicUsize::new(total_bytes),
            remain_bytes: AtomicUsize::new(total_bytes),
        };
        trace!("[PageAlloc] new, {:#?}", new_self);
        new_self
    }

    pub fn alloc_page(&self) -> *mut u8 {
        if self.remain_bytes.load(Ordering::Relaxed) < PAGE_SIZE {
            return std::ptr::null_mut();
        }
        self.remain_bytes.fetch_sub(PAGE_SIZE, Ordering::Relaxed);
        unsafe { alloc(self.page_layout()) }
    }

    pub unsafe fn dealloc_page(&self, page_ptr: *mut u8) {
        self.remain_bytes.fetch_add(PAGE_SIZE, Ordering::Relaxed);
        dealloc(page_ptr, self.page_layout())
    }

    /// Calculate current memory consumption.
    /// Return true if 90 percent capacity has been consumed.
    pub fn is_memory_low(&self) -> bool {
        let alloc_limit: usize = self.total_bytes.load(Ordering::Relaxed) / 10;
        if self.remain_bytes.load(Ordering::Relaxed) < alloc_limit {
            trace!("[PageAlloc] memory low, {:#?}", self);
            return true;
        }
        false
    }

    pub fn define_limit(&self, total_bytes: usize) {
        self.total_bytes.store(total_bytes, Ordering::Relaxed);
        self.remain_bytes.store(total_bytes, Ordering::Relaxed);
        trace!("[PageAlloc] define size limit, {:#?}", self);
    }

    #[inline]
    const fn page_layout(&self) -> Layout {
        unsafe { Layout::from_size_align_unchecked(PAGE_SIZE, PAGE_SIZE) }
    }
}

impl Debug for FixedSizePageAlloc {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(
            f,
            "FixedSizePageAlloc {{ total_bytes: {}, remain_bytes: {} }}",
            self.total_bytes.load(Ordering::Relaxed),
            self.remain_bytes.load(Ordering::Relaxed)
        )
    }
}

/// A macro to define a fixed-size allocator with total bytes.
/// The defined allocator implements the `PageAlloc` trait.
///
/// ```
/// impl_fixed_size_page_alloc! { MyFixedSizePageAlloc, 1024 }
/// ```
#[macro_export]
macro_rules! impl_fixed_size_page_alloc {
    ($page_alloc:ident, $total_bytes:expr) => {
        lazy_static::lazy_static! {
            /// A global fixed-size page allocator.
            /// The size limit should be user-defined later.
            pub static ref GLOBAL_FIXED_SIZE_PAGE_ALLOC: $crate::FixedSizePageAlloc
                = $crate::FixedSizePageAlloc::new(0);
        }

        GLOBAL_FIXED_SIZE_PAGE_ALLOC.define_limit($total_bytes);

        #[derive(Clone)]
        pub struct $page_alloc;

        impl $crate::PageAlloc for $page_alloc {
            fn alloc_page() -> *mut u8 {
                GLOBAL_FIXED_SIZE_PAGE_ALLOC.alloc_page()
            }

            unsafe fn dealloc_page(page_ptr: *mut u8) {
                GLOBAL_FIXED_SIZE_PAGE_ALLOC.dealloc_page(page_ptr);
            }

            fn register_low_memory_callback(f: impl Fn()) {
                if GLOBAL_FIXED_SIZE_PAGE_ALLOC.is_memory_low() {
                    f();
                }
            }

            fn is_memory_low() -> bool {
                GLOBAL_FIXED_SIZE_PAGE_ALLOC.is_memory_low()
            }
        }
    };
}
