/*
Occlum is a single-address-space library OS. Previously, userspace memory are divided for each process.
And all the memory are allocated when the process is created, which leads to a lot of wasted space and
complicated configuration.

In the current implementation, the whole userspace is managed as a memory pool that consists of chunks. There
are two kinds of chunks:
(1) Single VMA chunk: a chunk with only one VMA. Should be owned by exactly one process.
(2) Multi VMA chunk: a chunk with default chunk size and there could be a lot of VMAs in this chunk. Can be used
by different processes.

This design can help to achieve mainly two goals:
(1) Simplify the configuration: Users don't need to configure the process.default_mmap_size anymore. And multiple processes
running in the same Occlum instance can use dramatically different sizes of memory.
(2) Gain better performance: Two-level management(chunks & VMAs) reduces the time for finding, inserting, deleting, and iterating.

***************** Chart for Occlum User Space Memory Management ***************
 User Space VM Manager
┌──────────────────────────────────────────────────────────────┐
│                            VMManager                         │
│                                                              │
│  Chunks (in use): B-Tree Set                                 │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                      Multi VMA Chunk                   │  │
│  │                     ┌───────────────────────────────┐  │  │
│  │  Single VMA Chunk   │          ChunkManager         │  │  │
│  │  ┌──────────────┐   │                               │  │  │
│  │  │              │   │  VMAs (in use): Red Black Tree│  │  │
│  │  │    VMArea    │   │  ┌─────────────────────────┐  │  │  │
│  │  │              │   │  │                         │  │  │  │
│  │  └──────────────┘   │  │  ┌──────┐ ┌────┐ ┌────┐ │  │  │  │
│  │                     │  │  │ VMA  │ │VMA │ │VMA │ │  │  │  │
│  │  Single VMA Chunk   │  │  └──────┘ └────┘ └────┘ │  │  │  │
│  │  ┌──────────────┐   │  │                         │  │  │  │
│  │  │              │   │  └─────────────────────────┘  │  │  │
│  │  │    VMArea    │   │                               │  │  │
│  │  │              │   │                               │  │  │
│  │  └──────────────┘   │   Free Manager (free)         │  │  │
│  │                     │   ┌────────────────────────┐  │  │  │
│  │  Single VMA Chunk   │   │                        │  │  │  │
│  │  ┌──────────────┐   │   │   VMFreeSpaceManager   │  │  │  │
│  │  │              │   │   │                        │  │  │  │
│  │  │    VMArea    │   │   └────────────────────────┘  │  │  │
│  │  │              │   │                               │  │  │
│  │  └──────────────┘   └───────────────────────────────┘  │  │
│  │                                                        │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  Free Manager (free)                                         │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                                                        │  │
│  │                   VMFreeSpaceManager                   │  │
│  │                                                        │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
*/

use super::*;
use fs::{AsyncInodeExt, FileDesc, FileRef, INode};
use process::{Process, ProcessRef};
use std::fmt;

mod chunk;
mod free_space_manager;
mod process_vm;
mod user_space_vm;
mod vm_area;
mod vm_chunk_manager;
mod vm_layout;
mod vm_manager;
mod vm_perms;
mod vm_range;
mod vm_util;

use self::vm_layout::VMLayout;

pub use self::chunk::{Chunk, ChunkRef, ChunkType};
pub use self::process_vm::{MMapFlags, MRemapFlags, MSyncFlags, ProcessVM, ProcessVMBuilder};
pub use self::user_space_vm::{free_user_space, USER_SPACE_VM_MANAGER};
pub use self::vm_area::VMArea;
pub use self::vm_perms::VMPerms;
pub use self::vm_range::VMRange;
pub use self::vm_util::{VMInitializer, VMMapOptionsBuilder};

pub async fn do_mmap(
    addr: usize,
    size: usize,
    perms: VMPerms,
    flags: MMapFlags,
    fd: FileDesc,
    offset: usize,
) -> Result<usize> {
    if flags.contains(MMapFlags::MAP_ANONYMOUS) {
        debug!(
            "mmap: addr: {:#x}, size: {:#x}, perms: {:?}, flags: {:?}",
            addr, size, perms, flags,
        );
    } else {
        debug!(
            "mmap: addr: {:#x}, size: {:#x}, perms: {:?}, flags: {:?}, fd: {:?}, offset: {:?}",
            addr, size, perms, flags, fd, offset
        );
    }

    current!()
        .vm()
        .mmap(addr, size, perms, flags, fd, offset)
        .await
}

pub async fn do_munmap(addr: usize, size: usize) -> Result<()> {
    debug!("munmap: addr: {:#x}, size: {:#x}", addr, size);
    let current = current!();
    current!().vm().munmap(addr, size).await
}

pub async fn do_mremap(
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: MRemapFlags,
) -> Result<usize> {
    debug!(
        "mremap: old_addr: {:#x}, old_size: {:#x}, new_size: {:#x}, flags: {:?}",
        old_addr, old_size, new_size, flags
    );
    current!()
        .vm()
        .mremap(old_addr, old_size, new_size, flags)
        .await
}

pub async fn do_mprotect(addr: usize, size: usize, perms: VMPerms) -> Result<()> {
    debug!(
        "mprotect: addr: {:#x}, size: {:#x}, perms: {:?}",
        addr, size, perms
    );
    current!().vm().mprotect(addr, size, perms).await
}

pub async fn do_brk(addr: usize) -> Result<usize> {
    debug!("brk: addr: {:#x}", addr);
    current!().vm().brk(addr).await
}

pub async fn do_msync(addr: usize, size: usize, flags: MSyncFlags) -> Result<()> {
    debug!(
        "msync: addr: {:#x}, size: {:#x}, flags: {:?}",
        addr, size, flags
    );
    if flags.contains(MSyncFlags::MS_INVALIDATE) {
        return_errno!(EINVAL, "not support MS_INVALIDATE");
    }
    if flags.contains(MSyncFlags::MS_ASYNC) {
        warn!("not support MS_ASYNC");
    }
    current!().vm().msync(addr, size).await
}

pub const PAGE_SIZE: usize = 4096;
