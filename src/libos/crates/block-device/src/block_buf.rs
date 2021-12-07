use core::ptr::NonNull;

use crate::prelude::*;

/// A buffer of a block.
pub struct BlockBuf {
    ptr: NonNull<u8>,
}

impl BlockBuf {
    /// The size of a block buffer.
    pub const SIZE: usize = BLOCK_SIZE;

    /// Create a block buffer from a pointer.
    ///
    /// # Safety
    ///
    /// The given pointer must point to a valid memory region of the size of
    /// a block and the block buffer should be the only way to mutate the memory.
    pub unsafe fn from_ptr(ptr: NonNull<u8>) -> Self {
        Self { ptr }
    }

    /// Create a block buffer from a given boxed slice.
    ///
    /// The boxed slice must the a length of `BLOCK_SIZE`. Note that before
    /// dropping the new `BlockBuf`, the user should call `into_boxed` to
    /// prevent memory leakage.
    pub fn from_boxed(buf: Box<[u8]>) -> Self {
        assert!(buf.len() == BLOCK_SIZE);
        let ptr = Box::into_raw(buf) as *mut u8;
        Self {
            ptr: NonNull::new(ptr).unwrap(),
        }
    }

    /// Convert the block buffer innto a boxed slice.
    ///
    /// # Safety
    ///
    /// The block buffer must be created with `from_boxed`. Otherwise, the method
    /// leads to undefined behaviors.
    pub unsafe fn into_boxed(mut self) -> Box<[u8]> {
        Box::from_raw(self.as_slice_mut())
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

// Safety. BlockBuf owns the memory pointed by the internal point. So it is Send.
unsafe impl Send for BlockBuf {}
unsafe impl Sync for BlockBuf {}
