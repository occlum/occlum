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

const DEFAULT_CHUNK_SIZE: usize = 16 * 1024 * 1024; // 16MB

use vm_range::*;
use vm_util::*;

use libc::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use spin::Mutex;
use std::collections::LinkedList;
use vm_chunk_manager::ChunkManager;

pub struct Allocator {
    inner: Mutex<LinkedList<ChunkManager>>, // Use linked list for fast insert/delete.
}

impl Allocator {
    // Initiate a untrusted memory allocator with default size.
    pub fn new() -> Self {
        let total_bytes = DEFAULT_CHUNK_SIZE;

        let inner = {
            let mut new_list = LinkedList::new();
            let chunk_manager = ChunkManager::new(total_bytes)
                .expect("Creating untrusted allocator instance failure");
            new_list.push_back(chunk_manager);
            Mutex::new(new_list)
        };

        Self { inner }
    }

    // Allocate a block of memory and return the start address in c style.
    // Use exactly like malloc from libc.
    pub unsafe fn alloc(&self, size: usize, align: Option<usize>) -> Result<*mut u8> {
        let align = align.unwrap_or(DEFAULT_ALIGNMENT);

        // find a free range in the chunk list
        for chunk_manager in self.inner.lock().iter_mut() {
            if size > *chunk_manager.free_size() {
                continue;
            }

            if let Ok(addr) = chunk_manager.alloc(size, align) {
                return Ok(addr as *mut u8);
            }
        }

        // if no free range found, create a new chunk and allocate from it.
        let start_addr = self.alloc_from_new_chunk(size, align)?;
        Ok(start_addr as *mut u8)
    }

    // Free the memory block with specified start address
    // For memory allocated with alloc, free must be called.
    // Use exactly like free from libc.
    pub unsafe fn free(&self, addr: *mut u8) {
        let mut chunk_list = self.inner.lock();
        let (idx, chunk) = chunk_list
            .iter_mut()
            .enumerate()
            .find(|(_, chunk)| chunk.contains(addr as usize))
            .expect("free failure");

        chunk.free(addr as usize).expect("free failure");

        // Keep at most one empty chunk in the list. And free other empty chunks:
        // If the chunk is empty, and all other chunks are in use, push the chunk to the front of the list.
        // If the chunk is empty and the front chunk is also empty, just remove this chunk.
        // If this is the last chunk, just keep it.
        if chunk.is_empty() && chunk_list.len() > 1 {
            let empty_chunk = chunk_list.remove(idx);
            if !chunk_list.front().unwrap().is_empty() {
                chunk_list.push_front(empty_chunk);
            }
        }
    }

    fn alloc_from_new_chunk(&self, size: usize, align: usize) -> Result<usize> {
        let total_bytes = size.max(DEFAULT_CHUNK_SIZE);

        let mut new_chunk = ChunkManager::new(total_bytes)?;
        let ret_addr = new_chunk.alloc(size, align)?;

        self.inner.lock().push_front(new_chunk); // Add chunk to the list head to be iterated earlier.
        Ok(ret_addr)
    }
}

impl Drop for Allocator {
    fn drop(&mut self) {
        debug!("[untrusted alloc] Drop allocator");
        self.inner.lock().iter().for_each(|chunk| {
            assert!(chunk.is_empty() == true);
            assert!(chunk.is_free_range(chunk.range()));
        });
    }
}
