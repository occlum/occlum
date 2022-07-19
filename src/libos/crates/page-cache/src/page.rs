use crate::prelude::*;
use block_device::BLOCK_SIZE;

use std::alloc::Layout;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// A page obtained from an allocator.
pub struct Page<A: PageAlloc> {
    ptr: NonNull<u8>,
    marker: PhantomData<A>,
}

pub const PAGE_SIZE: usize = BLOCK_SIZE;

// Safety. PageAlloc implements Send and Sync.
unsafe impl<A: PageAlloc> Send for Page<A> {}
unsafe impl<A: PageAlloc> Sync for Page<A> {}

impl<A: PageAlloc> Page<A> {
    /// Create a new page.
    ///
    /// Return `None` if page allocation fails.
    pub fn new() -> Option<Self> {
        let raw_ptr = A::alloc_page();
        let opt_ptr = NonNull::new(raw_ptr);
        opt_ptr.map(|nonull_ptr| Self {
            ptr: nonull_ptr,
            marker: PhantomData,
        })
    }

    /// Return a pointer to the underlying page buffer.
    #[inline]
    pub fn as_ptr(&self) -> NonNull<u8> {
        self.ptr
    }

    /// Return a slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), Self::size()) }
    }

    /// Return a mutable slice.
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), Self::size()) }
    }

    /// Return the page size.
    #[inline]
    pub const fn size() -> usize {
        PAGE_SIZE
    }

    /// Return the page align.
    #[inline]
    pub const fn align() -> usize {
        PAGE_SIZE
    }

    /// Return the page layout.
    #[inline]
    pub const fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::size(), Self::align()) }
    }
}

impl<A: PageAlloc> Drop for Page<A> {
    fn drop(&mut self) {
        unsafe {
            A::dealloc_page(self.ptr.as_ptr());
        }
    }
}
