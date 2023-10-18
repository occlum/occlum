use super::*;
use std::alloc::{AllocError, Allocator, Layout};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

/// An memory allocator for slices, backed by a fixed-size, untrusted buffer
///
/// Note about memory ordering:
/// Here buf_pos is used here for counting, not to synchronize access to other
/// shared variables. Fetch_update guarantees that the operation is on the
/// "latest" value. Therefore, `Relaxed` can be used in both single-threaded and
/// multi-threaded environments.
pub struct UntrustedSliceAlloc {
    /// The pointer to the untrusted buffer
    buf_ptr: *mut u8,
    /// The size of the untrusted buffer
    buf_size: usize,
    /// The next position to allocate new slice
    /// New slices must be allocated from [buf_ptr + buf_pos, buf_ptr + buf_size)
    buf_pos: AtomicUsize,
}

unsafe impl Send for UntrustedSliceAlloc {}

impl UntrustedSliceAlloc {
    pub fn new(buf_size: usize) -> Result<Self> {
        if buf_size == 0 {
            // Create a dummy object
            return Ok(Self {
                buf_ptr: std::ptr::null_mut(),
                buf_size: 0,
                buf_pos: AtomicUsize::new(0),
            });
        }

        let layout = Layout::from_size_align(buf_size, 1)?;
        let buf_ptr = unsafe { UNTRUSTED_ALLOC.allocate(layout)?.as_mut_ptr() };

        let buf_pos = AtomicUsize::new(0);
        Ok(Self {
            buf_ptr,
            buf_size,
            buf_pos,
        })
    }

    pub fn guard(&self) -> UntrustedSliceAllocGuard<'_> {
        UntrustedSliceAllocGuard { alloc: self }
    }

    fn new_slice(&self, src_slice: &[u8]) -> Result<UntrustedSlice> {
        let mut new_slice = self.new_slice_mut(src_slice.len())?;
        new_slice.read_from_slice(src_slice)?;
        Ok(new_slice)
    }

    fn new_slice_mut(&self, new_slice_len: usize) -> Result<UntrustedSlice> {
        let new_slice_ptr = {
            // Move self.buf_pos forward if enough space _atomically_.
            let old_pos = self
                .buf_pos
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old_pos| {
                    let new_pos = old_pos + new_slice_len;
                    if new_pos <= self.buf_size {
                        Some(new_pos)
                    } else {
                        None
                    }
                })
                .map_err(|e| errno!(ENOMEM, "No enough space"))?;
            unsafe { self.buf_ptr.add(old_pos) }
        };
        let new_slice = unsafe { std::slice::from_raw_parts_mut(new_slice_ptr, new_slice_len) };
        Ok(UntrustedSlice { slice: new_slice })
    }

    fn reset(&self) {
        self.buf_pos.store(0, Ordering::Relaxed);
    }
}

impl Drop for UntrustedSliceAlloc {
    fn drop(&mut self) {
        // Do nothing for the dummy case
        if self.buf_size == 0 {
            return;
        }

        let layout = Layout::from_size_align(self.buf_size, 1).unwrap();
        unsafe {
            UNTRUSTED_ALLOC.deallocate(NonNull::new(self.buf_ptr).unwrap(), layout);
        }
    }
}

impl Debug for UntrustedSliceAlloc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UntrustedSliceAlloc")
            .field("buf size", &self.buf_size)
            .field("buf pos", &self.buf_pos.load(Ordering::Relaxed))
            .finish()
    }
}

pub struct UntrustedSliceAllocGuard<'a> {
    alloc: &'a UntrustedSliceAlloc,
}

impl<'a> UntrustedSliceAllocGuard<'a> {
    pub fn new_slice(&self, src_slice: &[u8]) -> Result<UntrustedSlice> {
        self.alloc.new_slice(src_slice)
    }
    pub fn new_slice_mut(&self, new_slice_len: usize) -> Result<UntrustedSlice> {
        self.alloc.new_slice_mut(new_slice_len)
    }
}

impl<'a> Drop for UntrustedSliceAllocGuard<'a> {
    fn drop(&mut self) {
        self.alloc.reset();
    }
}

pub struct UntrustedSlice<'a> {
    slice: &'a mut [u8],
}

impl UntrustedSlice<'_> {
    pub fn read_from_slice(&mut self, src_slice: &[u8]) -> Result<()> {
        assert!(self.len() >= src_slice.len());

        #[cfg(not(feature = "hyper_mode"))]
        self[..src_slice.len()].copy_from_slice(src_slice);
        #[cfg(feature = "hyper_mode")]
        {
            let n = unsafe {
                libc::ocall::write_shared_buf(
                    self.as_mut_ptr() as *mut _,
                    src_slice.as_ptr() as *const _,
                    src_slice.len(),
                    0,
                )
            };
            match n {
                n if n < 0 => return_errno!(ENOMEM, "No enough space"),
                n if n as usize == src_slice.len() => {}
                _ => return_errno!(ENOMEM, "failed to fill whole buffer"),
            }
        }

        Ok(())
    }

    pub fn write_to_slice(&self, dest_slice: &mut [u8]) -> Result<()> {
        assert!(self.len() >= dest_slice.len());

        #[cfg(not(feature = "hyper_mode"))]
        dest_slice.copy_from_slice(&self[..dest_slice.len()]);
        #[cfg(feature = "hyper_mode")]
        {
            let n = unsafe {
                libc::ocall::read_shared_buf(
                    self.as_ptr() as *const _,
                    dest_slice.as_mut_ptr() as *mut _,
                    dest_slice.len(),
                    0,
                )
            };
            match n {
                n if n < 0 => return_errno!(ENOMEM, "No enough space"),
                n if n as usize == dest_slice.len() => {}
                _ => return_errno!(ENOMEM, "failed to write whole buffer"),
            }
        }

        Ok(())
    }
}

impl AsRef<[u8]> for UntrustedSlice<'_> {
    fn as_ref(&self) -> &[u8] {
        &**self
    }
}

impl AsMut<[u8]> for UntrustedSlice<'_> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut **self
    }
}

impl Deref for UntrustedSlice<'_> {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl DerefMut for UntrustedSlice<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}
