use crate::page::PAGE_SIZE;

use std::alloc::{alloc, dealloc, Layout};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

/// A page allocator that can allocate/deallocate pages from free memory
/// and monitor the amount of free memory.
pub trait PageAlloc: Send + Sync + Clone + 'static {
    /// Allocate a new page.
    fn alloc_page() -> *mut u8;

    /// Deallocate a page.
    /// The `page_ptr` must be a valid pointer obtained from `alloc_page`.
    unsafe fn dealloc_page(page_ptr: *mut u8);

    /// Triggered when memory is low.
    fn register_low_memory_callback(f: impl Fn() + Send + Sync + 'static);

    /// Whether the memory is consumed out.
    /// User can define own memory limit.
    fn is_memory_low() -> bool;
}

/// A test-purpose page allocator with fixed total size.
pub struct FixedSizePageAlloc {
    total_bytes: u64,
    remain_bytes: u64,
    on_mem_low: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
}

impl FixedSizePageAlloc {
    pub fn new(total_bytes: usize) -> Self {
        let new_self = Self {
            total_bytes: total_bytes as _,
            remain_bytes: total_bytes as _,
            on_mem_low: None,
        };
        trace!("[PageAlloc] new, {:#?}", new_self);
        new_self
    }

    pub fn alloc_page(&mut self) -> *mut u8 {
        if self.is_memory_low() {
            self.on_mem_low
                .as_ref()
                .map(|low_mem_callback| low_mem_callback());
            return std::ptr::null_mut();
        }
        self.remain_bytes = self.remain_bytes.saturating_sub(PAGE_SIZE as _);
        unsafe { alloc(self.page_layout()) }
    }

    pub unsafe fn dealloc_page(&mut self, page_ptr: *mut u8) {
        self.remain_bytes = self.remain_bytes.saturating_add(PAGE_SIZE as _);
        dealloc(page_ptr, self.page_layout())
    }

    /// Check whether memory consumption reaches a low watermark.
    pub fn is_memory_low(&self) -> bool {
        const ALLOC_LIMIT: u64 = (2 * PAGE_SIZE) as _;
        if self.remain_bytes < ALLOC_LIMIT {
            trace!("[PageAlloc] memory low, {:#?}", self);
            return true;
        }
        false
    }

    pub fn register_low_memory_callback(&mut self, f: impl Fn() + Send + Sync + 'static) {
        let _ = self.on_mem_low.insert(Arc::new(f));
    }

    pub fn raise_alloc_limit(&mut self, num_bytes: usize) {
        self.total_bytes = self.total_bytes.saturating_add(num_bytes as _);
        self.remain_bytes = self.remain_bytes.saturating_add(num_bytes as _);
        trace!("[PageAlloc] raise alloc limit, {:#?}", self);
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
            self.total_bytes, self.remain_bytes
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
            pub static ref GLOBAL_FIXED_SIZE_PAGE_ALLOC: spin::Mutex<$crate::FixedSizePageAlloc>
                = spin::Mutex::new($crate::FixedSizePageAlloc::new(0));
        }

        GLOBAL_FIXED_SIZE_PAGE_ALLOC
            .lock()
            .raise_alloc_limit($total_bytes);

        #[derive(Clone)]
        pub struct $page_alloc;

        impl $crate::PageAlloc for $page_alloc {
            fn alloc_page() -> *mut u8 {
                GLOBAL_FIXED_SIZE_PAGE_ALLOC.lock().alloc_page()
            }

            unsafe fn dealloc_page(page_ptr: *mut u8) {
                GLOBAL_FIXED_SIZE_PAGE_ALLOC.lock().dealloc_page(page_ptr);
            }

            fn register_low_memory_callback(f: impl Fn() + Send + Sync + 'static) {
                GLOBAL_FIXED_SIZE_PAGE_ALLOC
                    .lock()
                    .register_low_memory_callback(f)
            }

            fn is_memory_low() -> bool {
                GLOBAL_FIXED_SIZE_PAGE_ALLOC.lock().is_memory_low()
            }
        }
    };
}
