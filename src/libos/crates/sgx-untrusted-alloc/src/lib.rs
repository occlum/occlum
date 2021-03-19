#![no_std]

extern crate sgx_types;
#[macro_use]
extern crate sgx_tstd as std;
extern crate sgx_trts;

extern crate buddy_system_allocator;

use buddy_system_allocator::LockedHeap;
use sgx_trts::libc;
use std::alloc::Layout;
use std::prelude::v1::*;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;

static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

/// An memory allocator for slices, backed by a fixed-size, untrusted buffer
pub struct UntrustedAllocator {
    /// The pointer to the untrusted buffer
    buf_ptr: *mut u8,
    /// The size of the untrusted buffer
    buf_size: usize,
    /// The alignment of the untrusted buffer
    buf_align: usize,
    /// The next position to allocate new slice
    /// New slices must be allocated from [buf_ptr + buf_pos, buf_ptr + buf_size)
    buf_pos: AtomicUsize,
}

impl UntrustedAllocator {
    pub fn new(buf_size: usize, buf_align: usize) -> Result<Self, ()> {
        if buf_size == 0 {
            // Create a dummy object
            return Ok(Self {
                buf_ptr: std::ptr::null_mut(),
                buf_size: 0,
                buf_pos: AtomicUsize::new(0),
                buf_align,
            });
        }

        let layout = Layout::from_size_align(buf_size, buf_align).map_err(|_| ())?;

        let mut alloc = HEAP_ALLOCATOR.lock();

        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let size = 1024 * 1024 * 1024;
            let heap_ptr = unsafe { libc::ocall::malloc(size) };
            assert!(!heap_ptr.is_null());
            unsafe {
                alloc.init(heap_ptr as *const u8 as usize, size);
            }
        });

        let buf_ptr = alloc.alloc(layout)?.as_ptr();
        let buf_pos = AtomicUsize::new(0);
        Ok(Self {
            buf_ptr,
            buf_size,
            buf_pos,
            buf_align,
        })
    }

    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.buf_ptr
    }

    pub fn capacity(&self) -> usize {
        self.buf_size
    }

    pub fn new_slice(&self, src_slice: &[u8]) -> Result<&[u8], ()> {
        let new_slice = self.new_slice_mut(src_slice.len())?;
        new_slice.copy_from_slice(src_slice);
        Ok(new_slice)
    }

    pub fn new_slice_mut(&self, new_slice_len: usize) -> Result<&mut [u8], ()> {
        let new_slice_ptr = {
            // Move self.buf_pos forward if enough space _atomically_.
            let old_pos = self
                .buf_pos
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old_pos| {
                    let new_pos = old_pos + new_slice_len;
                    if new_pos <= self.buf_size {
                        Some(new_pos)
                    } else {
                        None
                    }
                })
                .map_err(|_| {
                    println!(
                        "No enough space in UntrustedAllocator, buf_size: {}",
                        self.buf_size
                    );
                    ()
                })?;
            unsafe { self.buf_ptr.add(old_pos) }
        };
        let new_slice = unsafe { std::slice::from_raw_parts_mut(new_slice_ptr, new_slice_len) };
        Ok(new_slice)
    }

    pub fn new_slice_mut_align(&self, new_slice_len: usize, align: usize) -> Result<&mut [u8], ()> {
        let new_slice_ptr = {
            // Move self.buf_pos forward if enough space _atomically_.
            let old_pos = self
                .buf_pos
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old_pos| {
                    let mut new_pos = old_pos + new_slice_len;
                    if new_pos % align != 0 {
                        new_pos += align - new_pos % align;
                    }
                    if new_pos <= self.buf_size {
                        Some(new_pos)
                    } else {
                        None
                    }
                })
                .map_err(|_| {
                    println!(
                        "No enough space in UntrustedAllocator, buf_size: {}",
                        self.buf_size
                    );
                    ()
                })?;
            unsafe { self.buf_ptr.add(old_pos) }
        };
        let new_slice = unsafe { std::slice::from_raw_parts_mut(new_slice_ptr, new_slice_len) };
        Ok(new_slice)
    }
}

impl Drop for UntrustedAllocator {
    fn drop(&mut self) {
        // Do nothing for the dummy case
        if self.buf_size == 0 {
            return;
        }

        let layout = Layout::from_size_align(self.buf_size, self.buf_align).unwrap();
        HEAP_ALLOCATOR
            .lock()
            .dealloc(NonNull::new(self.buf_ptr).unwrap(), layout);
    }
}

unsafe impl Send for UntrustedAllocator {}

unsafe impl Sync for UntrustedAllocator {}
