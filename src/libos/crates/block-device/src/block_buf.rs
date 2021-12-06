use core::ptr::NonNull;

use crate::prelude::*;

/// A buffer of a block.
pub struct BlockBuf {
    ptr: NonNull<u8>,
}

impl BlockBuf {
    /// The size of a block buffer.
    pub const SIZE: usize = BLOCK_SIZE;

    /// Create an uninitialized block buffer.
    ///
    /// # Safety
    ///
    /// The given pointer must point to a valid memory region of the size of
    /// a block.
    pub unsafe fn new_uninit(ptr: NonNull<u8>) -> Self {
        Self { ptr }
    }

    /// Create a zeroed block buffer.
    ///
    /// # Safety
    ///
    /// The given pointer must point to a valid memory region of the size of
    /// a block.
    pub unsafe fn new_zerod(ptr: NonNull<u8>) -> Self {
        let mut new_self = Self::new_uninit(ptr);
        new_self.as_slice_mut().fill(0);
        new_self
    }

    /// Returns a pointer to the underlying buffer.
    pub fn as_ptr(&self) -> NonNull<u8> {
        self.ptr
    }

    /// Return a slice.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr() as _, Self::SIZE) }
    }

    /// Return a mutable slice.
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), Self::SIZE) }
    }
}
