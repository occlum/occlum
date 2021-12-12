use core::ptr::NonNull;

use crate::prelude::*;

/// A buffer for block requests.
///
/// The size of `BlockBuf` is a multiple of block size.
pub struct BlockBuf {
    ptr: NonNull<u8>,
    len: usize,
}

impl BlockBuf {
    /// Create a block buffer from a pointer.
    ///
    /// # Safety
    ///
    /// The given pointer must point to a valid memory region of the size of
    /// a block and the block buffer should be the only way to mutate the memory.
    #[inline]
    pub unsafe fn from_ptr(ptr: NonNull<u8>, len: usize) -> Self {
        debug_assert!(
            len <= isize::MAX as usize,
            "attempt to create a buffer that covers at least half of the address space"
        );
        debug_assert!(
            len % BLOCK_SIZE == 0,
            "attempt to create a buffer whose size is not a multiple of block size"
        );
        Self { ptr, len }
    }

    /// Create a block buffer from a given boxed slice.
    ///
    /// The boxed slice must the a length of `BLOCK_SIZE`. Note that before
    /// dropping the new `BlockBuf`, the user should call `into_boxed` to
    /// prevent memory leakage.
    #[inline]
    pub fn from_boxed(buf: Box<[u8]>) -> Self {
        let len = buf.len();
        debug_assert!(
            len % BLOCK_SIZE == 0,
            "attempt to create a buffer whose size is not a multiple of block size"
        );
        let ptr = Box::into_raw(buf) as *mut u8;
        Self {
            ptr: NonNull::new(ptr).unwrap(),
            len,
        }
    }

    /// Convert the block buffer innto a boxed slice.
    ///
    /// # Safety
    ///
    /// The block buffer must be created with `from_boxed`. Otherwise, the method
    /// leads to undefined behaviors.
    #[inline]
    pub unsafe fn into_boxed(mut self) -> Box<[u8]> {
        Box::from_raw(self.as_slice_mut())
    }

    /// Returns the length of the buffer in bytes.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns the length of the buffer in blocks.
    #[inline]
    pub const fn num_blocks(&self) -> usize {
        self.len >> BLOCK_SIZE_LOG2
    }

    /// Returns a pointer to the underlying buffer.
    #[inline]
    pub const fn as_ptr(&self) -> NonNull<u8> {
        self.ptr
    }

    /// Return a slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr() as _, self.len) }
    }

    /// Return a mutable slice.
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

// Safety. BlockBuf owns the memory pointed by the internal point. So it is Send.
unsafe impl Send for BlockBuf {}
unsafe impl Sync for BlockBuf {}
