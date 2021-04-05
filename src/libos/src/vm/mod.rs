use super::*;
use fs::{FileDesc, FileRef};
use process::{Process, ProcessRef};
use std::fmt;

mod process_vm;
mod user_space_vm;
mod vm_area;
mod vm_layout;
mod vm_manager;
mod vm_perms;
mod vm_range;

use self::vm_layout::VMLayout;
use self::vm_manager::{VMManager, VMMapOptionsBuilder};

pub use self::process_vm::{MMapFlags, MRemapFlags, MSyncFlags, ProcessVM, ProcessVMBuilder};
pub use self::user_space_vm::USER_SPACE_VM_MANAGER;
pub use self::vm_perms::VMPerms;
pub use self::vm_range::VMRange;

pub fn do_mmap(
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

    current!().vm().mmap(addr, size, perms, flags, fd, offset)
}

pub fn do_munmap(addr: usize, size: usize) -> Result<()> {
    debug!("munmap: addr: {:#x}, size: {:#x}", addr, size);
    let current = current!();
    current!().vm().munmap(addr, size)
}

pub fn do_mremap(
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: MRemapFlags,
) -> Result<usize> {
    debug!(
        "mremap: old_addr: {:#x}, old_size: {:#x}, new_size: {:#x}, flags: {:?}",
        old_addr, old_size, new_size, flags
    );
    current!().vm().mremap(old_addr, old_size, new_size, flags)
}

pub fn do_mprotect(addr: usize, size: usize, perms: VMPerms) -> Result<()> {
    debug!(
        "mprotect: addr: {:#x}, size: {:#x}, perms: {:?}",
        addr, size, perms
    );
    current!().vm().mprotect(addr, size, perms)
}

pub fn do_brk(addr: usize) -> Result<usize> {
    debug!("brk: addr: {:#x}", addr);
    current!().vm().brk(addr)
}

pub fn do_msync(addr: usize, size: usize, flags: MSyncFlags) -> Result<()> {
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
    current!().vm().msync(addr, size)
}

pub const PAGE_SIZE: usize = 4096;
