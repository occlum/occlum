use super::*;

use crate::prelude::*;

mod free_space_manager;
mod vm_area;
mod vm_chunk_manager;
mod vm_range;
mod vm_util;

const PAGE_SIZE: usize = 4096;

// The address of a block returned by malloc or realloc in GNU systems is always a multiple of eight (or sixteen on 64-bit systems).
const DEFAULT_ALIGNMENT: usize = 16;

use vm_range::*;
use vm_util::*;

use libc::c_void;
use libc::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use spin::Mutex;
use vm_chunk_manager::ChunkManager as VMManager;
cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use libc::ocall::{mmap, munmap};
    } else {
        use libc::{mmap, munmap};
    }
}

pub struct Allocator {
    range: VMRange,
    inner: Mutex<VMManager>,
}

impl Allocator {
    // Initiate a untrusted memory allocator with size specified by user.
    pub fn new(size: usize) -> Self {
        let total_bytes = size;
        let start_address = {
            let addr = unsafe {
                mmap(
                    0 as *mut _,
                    total_bytes,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS,
                    0,
                    0,
                )
            };

            assert!(addr != libc::MAP_FAILED);

            let addr = addr as usize;
            assert!(addr.checked_add(total_bytes).is_some());
            addr
        };
        let range = VMRange::new(start_address, start_address + total_bytes)
            .expect("Creating untrusted allocator instance failure");
        let inner = Mutex::new(
            VMManager::from(range.start(), total_bytes).expect("creating inner structure failure"),
        );
        debug!(
            "[untrusted alloc] Initiate a new allocator, range = {:?}",
            range
        );
        Self { range, inner }
    }

    // Allocate a block of memory and return the start address in c style.
    // Use exactly like malloc from libc.
    pub unsafe fn alloc(&self, size: usize, align: Option<usize>) -> Result<*mut u8> {
        if size > self.range.size() {
            return_errno!(ENOMEM, "malloc size too big");
        }

        let align = align.unwrap_or(DEFAULT_ALIGNMENT);
        let start_addr = self.inner.lock().alloc(size, align)?;
        Ok(start_addr as *mut u8)
    }

    // Free the memory block with specified start address
    // For memory allocated with alloc, free must be called.
    // Use exactly like free from libc.
    pub unsafe fn free(&self, addr: *mut u8) {
        self.inner.lock().free(addr as usize).expect("free failure");
    }
}

impl Drop for Allocator {
    fn drop(&mut self) {
        debug!("[untrusted alloc] Drop allocator");
        debug_assert!(self.inner.lock().check_empty() == true);
        debug_assert!(self.inner.lock().is_free_range(&self.range));
        let ret = unsafe { munmap(self.range.start as *mut c_void, self.range.size()) };
        assert!(ret == 0);
    }
}
