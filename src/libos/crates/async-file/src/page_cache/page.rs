#[cfg(feature = "sgx")]
use lazy_static::lazy_static;
#[cfg(feature = "sgx")]
use sgx_untrusted_alloc::UntrustedAllocator;
use std::alloc::{alloc, Layout};
use std::cell::UnsafeCell;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;

#[cfg(feature = "sgx")]
lazy_static! {
    static ref U_ALLOC: UntrustedAllocator =
        UntrustedAllocator::new(1024 * 1024 * 512, 4096).unwrap();
}

pub struct Page {
    // TODO: for SGX, this buffer needs to be allocated from a different source.
    buf: UnsafeCell<*mut u8>,
}

unsafe impl Send for Page {}
unsafe impl Sync for Page {}

impl Page {
    pub fn new() -> Self {
        #[cfg(not(feature = "sgx"))]
        let ptr = unsafe { alloc(Layout::from_size_align_unchecked(Page::size(), 4096)) };
        #[cfg(feature = "sgx")]
        let ptr = U_ALLOC
            .new_slice_mut_align(Page::size(), 4096)
            .unwrap()
            .as_mut_ptr();
        let buf = UnsafeCell::new(ptr);
        Self { buf }
    }

    pub unsafe fn as_slice(&self) -> &[u8] {
        std::slice::from_raw_parts(self.as_ptr(), Self::size())
    }

    pub unsafe fn as_slice_mut(&self) -> &mut [u8] {
        std::slice::from_raw_parts_mut(self.as_mut_ptr(), Self::size())
    }

    pub fn as_ptr(&self) -> *const u8 {
        unsafe { *self.buf.get() }
    }

    pub fn as_mut_ptr(&self) -> *mut u8 {
        unsafe { *self.buf.get() }
    }

    pub const fn size() -> usize {
        4096
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn fill_slice() {
        let page = Page::new();
        let slice_mut = unsafe { page.as_slice_mut() };
        slice_mut.fill(0xab);
        assert!(slice_mut.iter().all(|b| *b == 0xab));
    }
}
