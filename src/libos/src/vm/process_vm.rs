use super::*;

use super::config;
use super::process::elf_file::{ElfFile, ProgramHeaderExt};
use super::user_space_vm::{UserSpaceVMManager, UserSpaceVMRange, USER_SPACE_VM_MANAGER};
use super::vm_manager::{
    VMInitializer, VMManager, VMMapAddr, VMMapOptions, VMMapOptionsBuilder, VMRemapOptions,
};
use super::vm_perms::VMPerms;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone)]
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
                        let segment_size = (segment.p_vaddr + segment.p_memsz) as usize;
                        let segment_align = segment.p_align as usize;
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
        let brk = AtomicUsize::new(heap_range.start());

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

        let mmap_manager = SgxMutex::new(mmap_manager);

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
        let mut empty_offset_vec: Vec<(usize, usize)> = Vec::with_capacity(3); // usally two loadable segments
        let mut empty_start_offset = 0;
        let mut empty_end_offset = 0;

        // Source buffer: ELF stored in the ELF file
        let elf_file_buf = elf_file.as_slice();
        // Init all loadable segements
        let loadable_segments = elf_file
            .program_headers()
            .filter(|segment| segment.loadable())
            .for_each(|segment| {
                let file_size = segment.p_filesz as usize;
                let file_offset = segment.p_offset as usize;
                let mem_addr = segment.p_vaddr as usize;
                let mem_size = segment.p_memsz as usize;
                debug_assert!(file_size <= mem_size);

                // The first file_size bytes are loaded from the ELF file,
                // the remaining (mem_size - file_size) bytes are zeros.
                elf_file.file_inode().read_at(
                    file_offset,
                    &mut elf_proc_buf[mem_addr..mem_addr + file_size],
                );

                empty_end_offset = mem_addr;
                empty_offset_vec.push((empty_start_offset, empty_end_offset));
                empty_start_offset = empty_end_offset + file_size;
            });

        empty_offset_vec.push((empty_start_offset, elf_proc_buf.len() - 1));

        // Set zero for the remain part of the buffer
        empty_offset_vec
            .iter()
            .for_each(|(start_offset, end_offset)| {
                for b in &mut elf_proc_buf[*start_offset..*end_offset] {
                    *b = 0;
                }
            });

        Ok(())
    }
}

/// The per-process virtual memory
#[derive(Debug)]
pub struct ProcessVM {
    mmap_manager: SgxMutex<VMManager>,
    elf_ranges: Vec<VMRange>,
    heap_range: VMRange,
    stack_range: VMRange,
    brk: AtomicUsize,
    // Memory safety notes: the process_range field must be the last one.
    //
    // Rust drops fields in the same order as they are declared. So by making
    // process_range the last field, we ensure that when all other fields are
    // dropped, their drop methods (if provided) can still access the memory
    // region represented by the process_range field.
    process_range: UserSpaceVMRange,
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
        self.brk.load(Ordering::SeqCst)
    }

    pub fn brk(&self, new_brk: usize) -> Result<usize> {
        let heap_start = self.heap_range.start();
        let heap_end = self.heap_range.end();

        if new_brk == 0 {
            return Ok(self.get_brk());
        } else if new_brk < heap_start {
            return_errno!(EINVAL, "New brk address is too low");
        } else if new_brk > heap_end {
            return_errno!(EINVAL, "New brk address is too high");
        }

        self.brk
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old_brk| Some(new_brk));
        Ok(new_brk)
    }

    pub fn mmap(
        &self,
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
                VMMapAddr::Force(addr)
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
                let file_ref = current!().file(fd)?;
                VMInitializer::LoadFromFile {
                    file: file_ref,
                    offset: offset,
                }
            }
        };
        // Only shared, file-backed memory mappings have write-back files
        let writeback_file = if flags.contains(MMapFlags::MAP_SHARED) {
            if let VMInitializer::LoadFromFile { file, offset } = &initializer {
                Some((file.clone(), *offset))
            } else {
                None
            }
        } else {
            None
        };
        let mmap_options = VMMapOptionsBuilder::default()
            .size(size)
            .addr(addr_option)
            .perms(perms)
            .initializer(initializer)
            .writeback_file(writeback_file)
            .build()?;
        let mmap_addr = self.mmap_manager.lock().unwrap().mmap(mmap_options)?;
        Ok(mmap_addr)
    }

    pub fn mremap(
        &self,
        old_addr: usize,
        old_size: usize,
        new_size: usize,
        flags: MRemapFlags,
    ) -> Result<usize> {
        if let Some(new_addr) = flags.new_addr() {
            if !self.process_range.range().contains(new_addr) {
                return_errno!(EINVAL, "new_addr is beyond valid memory range");
            }
        }

        let mremap_option = VMRemapOptions::new(old_addr, old_size, new_size, flags)?;
        self.mmap_manager.lock().unwrap().mremap(&mremap_option)
    }

    pub fn munmap(&self, addr: usize, size: usize) -> Result<()> {
        self.mmap_manager.lock().unwrap().munmap(addr, size)
    }

    pub fn mprotect(&self, addr: usize, size: usize, perms: VMPerms) -> Result<()> {
        let size = {
            if size == 0 {
                return Ok(());
            }
            align_up(size, PAGE_SIZE)
        };
        let protect_range = VMRange::new_with_size(addr, size)?;
        if !self.process_range.range().is_superset_of(&protect_range) {
            return_errno!(ENOMEM, "invalid range");
        }
        let mut mmap_manager = self.mmap_manager.lock().unwrap();

        // TODO: support mprotect vm regions in addition to mmap
        if !mmap_manager.range().is_superset_of(&protect_range) {
            warn!("Do not support mprotect memory outside the mmap region yet");
            return Ok(());
        }

        mmap_manager.mprotect(addr, size, perms)
    }

    pub fn msync(&self, addr: usize, size: usize) -> Result<()> {
        let sync_range = VMRange::new_with_size(addr, size)?;
        let mut mmap_manager = self.mmap_manager.lock().unwrap();
        mmap_manager.msync_by_range(&sync_range)
    }

    pub fn msync_by_file(&self, sync_file: &FileRef) {
        let mut mmap_manager = self.mmap_manager.lock().unwrap();
        mmap_manager.msync_by_file(sync_file);
    }

    // Return: a copy of the found region
    pub fn find_mmap_region(&self, addr: usize) -> Result<VMRange> {
        self.mmap_manager
            .lock()
            .unwrap()
            .find_mmap_region(addr)
            .map(|range_ref| *range_ref)
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MRemapFlags {
    None,
    MayMove,
    FixedAddr(usize),
}

impl MRemapFlags {
    pub fn from_raw(raw_flags: u32, new_addr: usize) -> Result<Self> {
        const MREMAP_NONE: u32 = 0;
        const MREMAP_MAYMOVE: u32 = 1;
        const MREMAP_FIXED: u32 = 3;

        #[deny(unreachable_patterns)]
        let flags = match raw_flags {
            MREMAP_NONE => Self::None,
            MREMAP_MAYMOVE => Self::MayMove,
            MREMAP_FIXED => Self::FixedAddr(new_addr),
            _ => return_errno!(EINVAL, "unsupported flags"),
        };
        Ok(flags)
    }

    pub fn new_addr(&self) -> Option<usize> {
        match self {
            MRemapFlags::FixedAddr(new_addr) => Some(*new_addr),
            _ => None,
        }
    }
}

impl Default for MRemapFlags {
    fn default() -> Self {
        MRemapFlags::None
    }
}

bitflags! {
    pub struct MSyncFlags : u32 {
        const MS_ASYNC      = 0x1;
        const MS_INVALIDATE = 0x2;
        const MS_SYNC       = 0x4;
    }
}

impl MSyncFlags {
    pub fn from_u32(bits: u32) -> Result<Self> {
        let flags =
            MSyncFlags::from_bits(bits).ok_or_else(|| errno!(EINVAL, "containing unknown bits"))?;
        if flags.contains(Self::MS_ASYNC | Self::MS_SYNC) {
            return_errno!(EINVAL, "must be either sync or async");
        }
        Ok(flags)
    }
}
