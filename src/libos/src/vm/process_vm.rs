use super::*;
use super::vm_manager::{VMRange, VMManager, VMMapOptionsBuilder, VMMapOptions, VMMapAddr, VMInitializer};
use super::user_space_vm::{UserSpaceVMManager, UserSpaceVMRange, USER_SPACE_VM_MANAGER};
use std::slice;

/// The per-process virtual memory
#[derive(Debug)]
pub struct ProcessVM {
    process_range: UserSpaceVMRange,
    code_range: VMRange,
    data_range: VMRange,
    heap_range: VMRange,
    stack_range: VMRange,
    brk: usize,
    mmap_manager: VMManager,
}

impl Default for ProcessVM {
    fn default() -> ProcessVM {
        ProcessVM {
            process_range: USER_SPACE_VM_MANAGER.alloc_dummy(),
            code_range: VMRange::default(),
            data_range: VMRange::default(),
            heap_range: VMRange::default(),
            stack_range: VMRange::default(),
            brk: 0,
            mmap_manager: VMManager::default(),
        }
    }
}

impl ProcessVM {
    pub fn new(
        code_size: usize,
        data_size: usize,
        heap_size: usize,
        stack_size: usize,
        mmap_size: usize,
    ) -> Result<ProcessVM, Error> {
        let process_range = {
            let vm_range_size = code_size + data_size + heap_size + stack_size + mmap_size;
            USER_SPACE_VM_MANAGER.alloc(vm_range_size)?
        };
        let process_addr = process_range.range().start();

        let range_sizes = vec![code_size, data_size, heap_size, stack_size];
        let mut curr_addr = process_addr;
        let mut ranges = Vec::new();
        for range_size in &range_sizes {
            let range_start = curr_addr;
            let range_end = curr_addr + range_size;
            let range = VMRange::from(range_start, range_end)?;
            ranges.push(range);

            curr_addr = range_end;
        }
        let code_range = *&ranges[0];
        let data_range = *&ranges[1];
        let heap_range = *&ranges[2];
        let stack_range = *&ranges[3];
        unsafe {
            fill_zeros(code_range.start(), code_range.size());
            fill_zeros(data_range.start(), data_range.size());
        }

        let brk = heap_range.start();

        let mmap_addr = stack_range.end();
        let mmap_manager = VMManager::from(mmap_addr, mmap_size)?;

        Ok(ProcessVM {
            process_range,
            code_range,
            data_range,
            heap_range,
            stack_range,
            brk,
            mmap_manager,
        })
    }

    pub fn get_process_range(&self) -> &VMRange {
        self.process_range.range()
    }

    pub fn get_code_range(&self) -> &VMRange {
        &self.code_range
    }

    pub fn get_data_range(&self) -> &VMRange {
        &self.data_range
    }

    pub fn get_heap_range(&self) -> &VMRange {
        &self.heap_range
    }

    pub fn get_stack_range(&self) -> &VMRange {
        &self.stack_range
    }

    pub fn get_base_addr(&self) -> usize {
        self.get_process_range().start()
    }

    pub fn get_stack_top(&self) -> usize {
        self.get_stack_range().end()
    }

    pub fn get_brk(&self) -> usize {
        self.brk
    }

    pub fn brk(&mut self, new_brk: usize) -> Result<usize, Error> {
        let heap_start = self.heap_range.start();
        let heap_end = self.heap_range.end();

        if new_brk == 0 {
            return Ok(self.get_brk());
        } else if new_brk < heap_start {
            return errno!(EINVAL, "New brk address is too low");
        } else if new_brk > heap_end {
            return errno!(EINVAL, "New brk address is too high");
        }

        if self.brk < new_brk {
            unsafe { fill_zeros(self.brk, new_brk - self.brk) };
        }

        self.brk = new_brk;
        return Ok(new_brk);
    }

    pub fn mmap(
        &mut self,
        addr: usize,
        size: usize,
        perms: VMPerms,
        flags: MMapFlags,
        fd: FileDesc,
        offset: usize
    ) -> Result<usize, Error> {
        let addr_option = {
            if flags.contains(MMapFlags::MAP_FIXED) {
                if !self.process_range.range().contains(addr) {
                    return errno!(EINVAL, "Beyond valid memory range");
                }
                VMMapAddr::Fixed(addr)
            } else {
                if addr == 0 {
                    VMMapAddr::Any
                } else {
                    VMMapAddr::Hint(addr)
                }
            }
        };
        let initializer = {
            if flags.contains(MMapFlags::MAP_ANONYMOUS) {
                VMInitializer::FillZeros()
            } else {
                let current_ref = get_current();
                let current_process = current_ref.lock().unwrap();
                let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
                VMInitializer::LoadFromFile { file: file_ref, offset: offset }
            }
        };
        let mmap_options = VMMapOptionsBuilder::default()
            .size(size)
            .addr(addr_option)
            .initializer(initializer)
            .build()?;
        let mmap_addr = self.mmap_manager.mmap(&mmap_options)?;
        Ok(mmap_addr)
    }

    pub fn munmap(&mut self, addr: usize, size: usize) -> Result<(), Error> {
        self.mmap_manager.munmap(addr, size)
    }
}


bitflags! {
    pub struct MMapFlags : u32 {
        const MAP_FILE            = 0x0;
        const MAP_SHARED          = 0x1;
        const MAP_PRIVATE         = 0x2;
        const MAP_SHARED_VALIDATE = 0x3;
        const MAP_TYPE            = 0xf;
        const MAP_FIXED           = 0x10;
        const MAP_ANONYMOUS       = 0x20;
        const MAP_GROWSDOWN       = 0x100;
        const MAP_DENYWRITE       = 0x800;
        const MAP_EXECUTABLE      = 0x1000;
        const MAP_LOCKED          = 0x2000;
        const MAP_NORESERVE       = 0x4000;
        const MAP_POPULATE        = 0x8000;
        const MAP_NONBLOCK        = 0x10000;
        const MAP_STACK           = 0x20000;
        const MAP_HUGETLB         = 0x40000;
        const MAP_SYNC            = 0x80000;
        const MAP_FIXED_NOREPLACE = 0x100000;
    }
}

impl MMapFlags {
    pub fn from_u32(bits: u32) -> Result<MMapFlags, Error> {
        // TODO: detect non-supporting flags
        MMapFlags::from_bits(bits)
            .ok_or_else(|| (Errno::EINVAL, "Unknown mmap flags").into())
    }
}


bitflags! {
    pub struct VMPerms : u32 {
        const READ        = 0x1;
        const WRITE       = 0x2;
        const EXEC        = 0x4;
    }
}

impl VMPerms {
    pub fn can_read(&self) -> bool {
        self.contains(VMPerms::READ)
    }

    pub fn can_write(&self) -> bool {
        self.contains(VMPerms::WRITE)
    }

    pub fn can_execute(&self) -> bool {
        self.contains(VMPerms::EXEC)
    }

    pub fn from_u32(bits: u32) -> Result<VMPerms, Error> {
        VMPerms::from_bits(bits)
            .ok_or_else(|| (Errno::EINVAL, "Unknown permission bits").into())
    }
}


unsafe fn fill_zeros(addr: usize, size: usize) {
    let ptr = addr as *mut u8;
    let buf = slice::from_raw_parts_mut(ptr, size);
    for b in buf {
        *b = 0;
    }
}

