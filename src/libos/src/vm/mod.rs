use prelude::*;
use process::{get_current, Process, ProcessRef};
use fs::{FileDesc, FileRef, File};
use std::fmt;

mod vm_manager;
mod user_space_vm;
mod process_vm;

pub use self::process_vm::{ProcessVM, MMapFlags, VMPerms};
pub use self::vm_manager::{VMRange};

pub fn do_mmap(
    addr: usize,
    size: usize,
    perms: VMPerms,
    flags: MMapFlags,
    fd: FileDesc,
    offset: usize,
) -> Result<usize, Error> {
    if flags.contains(MMapFlags::MAP_ANONYMOUS) {
        info!(
            "mmap: addr: {:#x}, size: {:#x}, perms: {:?}, flags: {:?}",
            addr, size, perms, flags,
        );
    } else {
        info!(
            "mmap: addr: {:#x}, size: {:#x}, perms: {:?}, flags: {:?}, fd: {:?}, offset: {:?}",
            addr, size, perms, flags, fd, offset
        );
    }

    let mut current_vm_ref = {
        let current_ref = get_current();
        let current_process = current_ref.lock().unwrap();
        current_process.get_vm().clone()
    };
    let mut current_vm = current_vm_ref.lock().unwrap();
    current_vm.mmap(addr, size, perms, flags, fd, offset)
}

pub fn do_munmap(addr: usize, size: usize) -> Result<(), Error> {
    info!("munmap: addr: {:#x}, size: {:#x}", addr, size);
    let mut current_vm_ref = {
        let current_ref = get_current();
        let current_process = current_ref.lock().unwrap();
        current_process.get_vm().clone()
    };
    let mut current_vm = current_vm_ref.lock().unwrap();
    current_vm.munmap(addr, size)
}

pub fn do_brk(addr: usize) -> Result<usize, Error> {
    info!("brk: addr: {:#x}", addr);
    let current_ref = get_current();
    let current_process = current_ref.lock().unwrap();
    let current_vm_ref = current_process.get_vm();
    let mut current_vm = current_vm_ref.lock().unwrap();
    current_vm.brk(addr)
}

pub const PAGE_SIZE: usize = 4096;
