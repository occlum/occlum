use super::*;

use super::config;
use super::process::{ElfFile, ProgramHeaderExt};
use super::user_space_vm::{UserSpaceVMManager, UserSpaceVMRange, USER_SPACE_VM_MANAGER};
use super::vm_manager::{VMInitializer, VMManager, VMMapAddr, VMMapOptions, VMMapOptionsBuilder};

#[derive(Debug)]
pub struct ProcessVMBuilder<'a, 'b> {
    elfs: Vec<&'b ElfFile<'a>>,
    heap_size: Option<usize>,
    stack_size: Option<usize>,
    mmap_size: Option<usize>,
}

impl<'a, 'b> ProcessVMBuilder<'a, 'b> {
    pub fn new(elfs: Vec<&'b ElfFile<'a>>) -> ProcessVMBuilder<'a, 'b> {
        ProcessVMBuilder {
            elfs: elfs,
            heap_size: None,
            stack_size: None,
            mmap_size: None,
        }
    }

    pub fn set_heap_size(&mut self, heap_size: usize) -> &mut Self {
        self.heap_size = Some(heap_size);
        self
    }

    pub fn set_stack_size(&mut self, stack_size: usize) -> &mut Self {
        self.stack_size = Some(stack_size);
        self
    }

    pub fn set_mmap_size(&mut self, mmap_size: usize) -> &mut Self {
        self.mmap_size = Some(mmap_size);
        self
    }

    pub fn build(self) -> Result<ProcessVM> {
        self.validate()?;

        let heap_size = self
            .heap_size
            .unwrap_or(config::LIBOS_CONFIG.process.default_heap_size);
        let stack_size = self
            .stack_size
            .unwrap_or(config::LIBOS_CONFIG.process.default_stack_size);
        let mmap_size = self
            .mmap_size
            .unwrap_or(config::LIBOS_CONFIG.process.default_mmap_size);

        // Before allocating memory, let's first calcualte how much memory
        // we need in total by iterating the memory layouts required by
        // all the memory regions
        let elf_layouts: Vec<VMLayout> = self
            .elfs
            .iter()
            .map(|elf| {
                elf.program_headers()
                    .filter(|segment| segment.loadable())
                    .fold(VMLayout::new_empty(), |mut elf_layout, segment| {
                        let segment_size = (segment.virtual_addr() + segment.mem_size()) as usize;
                        let segment_align = segment.align() as usize;
                        let segment_layout = VMLayout::new(segment_size, segment_align).unwrap();
                        elf_layout.extend(&segment_layout);
                        elf_layout
                    })
            })
            .collect();
        let other_layouts = vec![
            VMLayout::new(heap_size, PAGE_SIZE)?,
            VMLayout::new(stack_size, PAGE_SIZE)?,
            VMLayout::new(mmap_size, PAGE_SIZE)?,
        ];
        let process_layout = elf_layouts.iter().chain(other_layouts.iter()).fold(
            VMLayout::new_empty(),
            |mut process_layout, sub_layout| {
                process_layout.extend(&sub_layout);
                process_layout
            },
        );

        // Now that we end up with the memory layout required by the process,
        // let's allocate the memory for the process
        let process_range = {
            // TODO: ensure alignment through USER_SPACE_VM_MANAGER, not by
            // preserving extra space for alignment
            USER_SPACE_VM_MANAGER.alloc(process_layout.align() + process_layout.size())?
        };
        let process_base = process_range.range().start();

        // Init the memory for ELFs in the process
        let elf_ranges: Vec<VMRange> = {
            let mut min_elf_start = process_base;
            elf_layouts
                .iter()
                .map(|elf_layout| {
                    let new_elf_range = VMRange::new_with_layout(elf_layout, min_elf_start);
                    min_elf_start = new_elf_range.end();
                    new_elf_range
                })
                .collect()
        };
        self.elfs
            .iter()
            .zip(elf_ranges.iter())
            .try_for_each(|(elf, elf_range)| Self::init_elf_memory(elf_range, elf))?;

        // Init the heap memory in the process
        let heap_layout = &other_layouts[0];
        let heap_min_start = {
            let last_elf_range = elf_ranges.iter().last().unwrap();
            last_elf_range.end()
        };
        let heap_range = VMRange::new_with_layout(heap_layout, heap_min_start);
        unsafe {
            let heap_buf = heap_range.as_slice_mut();
            for b in heap_buf {
                *b = 0;
            }
        }
        let brk = heap_range.start();

        // Init the stack memory in the process
        let stack_layout = &other_layouts[1];
        let stack_min_start = heap_range.end();
        let stack_range = VMRange::new_with_layout(stack_layout, stack_min_start);
        // Note: we do not need to fill zeros for stack

        // Init the mmap memory in the process
        let mmap_layout = &other_layouts[2];
        let mmap_min_start = stack_range.end();
        let mmap_range = VMRange::new_with_layout(mmap_layout, mmap_min_start);
        let mmap_manager = VMManager::from(mmap_range.start(), mmap_range.size())?;
        // Note: we do not need to fill zeros of the mmap region.
        // VMManager will fill zeros (if necessary) on mmap.

        debug_assert!(elf_ranges
            .iter()
            .all(|elf_range| process_range.range().is_superset_of(elf_range)));
        debug_assert!(process_range.range().is_superset_of(&heap_range));
        debug_assert!(process_range.range().is_superset_of(&stack_range));
        debug_assert!(process_range.range().is_superset_of(&mmap_range));

        Ok(ProcessVM {
            process_range,
            elf_ranges,
            heap_range,
            stack_range,
            brk,
            mmap_manager,
        })
    }

    fn validate(&self) -> Result<()> {
        let validate_size = |size_opt| -> Result<()> {
            if let Some(size) = size_opt {
                if size == 0 || size % PAGE_SIZE != 0 {
                    return_errno!(EINVAL, "invalid size");
                }
            }
            Ok(())
        };
        validate_size(self.heap_size)?;
        validate_size(self.stack_size)?;
        validate_size(self.mmap_size)?;
        Ok(())
    }

    fn init_elf_memory(elf_range: &VMRange, elf_file: &ElfFile) -> Result<()> {
        // Destination buffer: ELF appeared in the process
        let elf_proc_buf = unsafe { elf_range.as_slice_mut() };
        // Source buffer: ELF stored in the ELF file
        let elf_file_buf = elf_file.as_slice();
        // Init all loadable segements
        let loadable_segments = elf_file
            .program_headers()
            .filter(|segment| segment.loadable())
            .for_each(|segment| {
                let file_size = segment.file_size() as usize;
                let file_offset = segment.offset() as usize;
                let mem_addr = segment.virtual_addr() as usize;
                let mem_size = segment.mem_size() as usize;
                debug_assert!(file_size <= mem_size);

                // The first file_size bytes are loaded from the ELF file
                elf_proc_buf[mem_addr..mem_addr + file_size]
                    .copy_from_slice(&elf_file_buf[file_offset..file_offset + file_size]);
                // The remaining (mem_size - file_size) bytes are zeros
                for b in &mut elf_proc_buf[mem_addr + file_size..mem_addr + mem_size] {
                    *b = 0;
                }
            });
        Ok(())
    }
}

/// The per-process virtual memory
#[derive(Debug)]
pub struct ProcessVM {
    process_range: UserSpaceVMRange,
    elf_ranges: Vec<VMRange>,
    heap_range: VMRange,
    stack_range: VMRange,
    brk: usize,
    mmap_manager: VMManager,
}

impl Default for ProcessVM {
    fn default() -> ProcessVM {
        ProcessVM {
            process_range: USER_SPACE_VM_MANAGER.alloc_dummy(),
            elf_ranges: Default::default(),
            heap_range: Default::default(),
            stack_range: Default::default(),
            brk: Default::default(),
            mmap_manager: Default::default(),
        }
    }
}

impl ProcessVM {
    pub fn get_process_range(&self) -> &VMRange {
        self.process_range.range()
    }

    pub fn get_elf_ranges(&self) -> &[VMRange] {
        &self.elf_ranges
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

    pub fn get_stack_base(&self) -> usize {
        self.get_stack_range().end()
    }

    pub fn get_stack_limit(&self) -> usize {
        self.get_stack_range().start()
    }

    pub fn get_brk(&self) -> usize {
        self.brk
    }

    pub fn brk(&mut self, new_brk: usize) -> Result<usize> {
        let heap_start = self.heap_range.start();
        let heap_end = self.heap_range.end();

        if new_brk == 0 {
            return Ok(self.get_brk());
        } else if new_brk < heap_start {
            return_errno!(EINVAL, "New brk address is too low");
        } else if new_brk > heap_end {
            return_errno!(EINVAL, "New brk address is too high");
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
        offset: usize,
    ) -> Result<usize> {
        let addr_option = {
            if flags.contains(MMapFlags::MAP_FIXED) {
                if !self.process_range.range().contains(addr) {
                    return_errno!(EINVAL, "Beyond valid memory range");
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
                VMInitializer::LoadFromFile {
                    file: file_ref,
                    offset: offset,
                }
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

    pub fn munmap(&mut self, addr: usize, size: usize) -> Result<()> {
        self.mmap_manager.munmap(addr, size)
    }

    pub fn find_mmap_region(&self, addr: usize) -> Result<&VMRange> {
        self.mmap_manager.find_mmap_region(addr)
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
    pub fn from_u32(bits: u32) -> Result<MMapFlags> {
        // TODO: detect non-supporting flags
        MMapFlags::from_bits(bits).ok_or_else(|| errno!(EINVAL, "unknown mmap flags"))
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

    pub fn from_u32(bits: u32) -> Result<VMPerms> {
        VMPerms::from_bits(bits).ok_or_else(|| errno!(EINVAL, "unknown permission bits"))
    }
}

unsafe fn fill_zeros(addr: usize, size: usize) {
    let ptr = addr as *mut u8;
    let buf = std::slice::from_raw_parts_mut(ptr, size);
    for b in buf {
        *b = 0;
    }
}
