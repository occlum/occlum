use super::*;

use super::chunk::*;
use super::config;
use super::process::elf_file::{ElfFile, ProgramHeaderExt};
use super::user_space_vm::USER_SPACE_VM_MANAGER;
use super::vm_area::VMArea;
use super::vm_perms::VMPerms;
use super::vm_util::{
    FileBacked, VMInitializer, VMMapAddr, VMMapOptions, VMMapOptionsBuilder, VMRemapOptions,
};

use async_rt::sync::{RwLock as AsyncRwLock, RwLockWriteGuard as AsyncRwLockWriteGuard};
use std::collections::HashSet;
use util::sync::RwLockWriteGuard;

#[derive(Debug, Clone)]
pub struct ProcessVMBuilder<'a, 'b> {
    elfs: Vec<&'b ElfFile<'a>>,
    heap_size: Option<usize>,
    stack_size: Option<usize>,
}

impl<'a, 'b> ProcessVMBuilder<'a, 'b> {
    pub fn new(elfs: Vec<&'b ElfFile<'a>>) -> ProcessVMBuilder<'a, 'b> {
        ProcessVMBuilder {
            elfs: elfs,
            heap_size: None,
            stack_size: None,
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

    pub async fn build(self) -> Result<ProcessVM> {
        self.validate()?;

        let heap_size = self
            .heap_size
            .unwrap_or(config::LIBOS_CONFIG.process.default_heap_size);
        let stack_size = self
            .stack_size
            .unwrap_or(config::LIBOS_CONFIG.process.default_stack_size);

        // Before allocating memory, let's first calculate how much memory
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

        // Make heap and stack page-aligned for simplicity
        // Heap and stack can be 16-byte aligned, but with address space layout randomization, this can make many
        // assertion failure.
        let other_layouts = vec![
            VMLayout::new(heap_size, PAGE_SIZE)?,
            VMLayout::new(stack_size, PAGE_SIZE)?,
        ];
        let process_layout = elf_layouts.iter().chain(other_layouts.iter()).fold(
            VMLayout::new_empty(),
            |mut process_layout, sub_layout| {
                process_layout.add(&sub_layout);
                process_layout
            },
        );

        // Now that we end up with the memory layout required by the process,
        // let's allocate the memory for the process
        let mut chunks = HashSet::new();
        // Init the memory for ELFs in the process
        let mut elf_ranges = Vec::with_capacity(2);
        for (elf_layout, elf_file) in elf_layouts.iter().zip(self.elfs.iter()) {
            let vm_option = {
                let vm_option = VMMapOptionsBuilder::default()
                    .size(elf_layout.size())
                    .align(elf_layout.align())
                    .perms(VMPerms::ALL) // set it to read | write | exec for simplicity
                    .initializer(VMInitializer::ElfSpecific {
                        elf_file: elf_file.file_ref().clone(),
                    })
                    .build();
                if vm_option.is_err() {
                    self.handle_error_when_init(&chunks).await;
                }
                vm_option?
            };
            let (elf_range, chunk_ref) = {
                let res = USER_SPACE_VM_MANAGER.alloc(&vm_option).await;
                if res.is_err() {
                    self.handle_error_when_init(&chunks).await;
                };
                res?
            };
            debug_assert!(elf_range.start() % elf_layout.align() == 0);
            chunks.insert(chunk_ref);
            if let Err(e) = Self::init_elf_memory(&elf_range, elf_file).await {
                self.handle_error_when_init(&chunks).await;
                return Err(e);
            }
            trace!("elf range = {:?}", elf_range);
            elf_ranges.push(elf_range);
        }

        // Init the heap memory in the process
        let heap_layout = &other_layouts[0];
        let vm_option = {
            let vm_option = VMMapOptionsBuilder::default()
                .size(heap_layout.size())
                .align(heap_layout.align())
                .perms(VMPerms::READ | VMPerms::WRITE)
                .build();
            if vm_option.is_err() {
                self.handle_error_when_init(&chunks).await
            }
            vm_option?
        };

        let (heap_range, chunk_ref) = {
            let res = USER_SPACE_VM_MANAGER.alloc(&vm_option).await;
            if res.is_err() {
                self.handle_error_when_init(&chunks).await
            }
            res?
        };
        debug_assert!(heap_range.start() % heap_layout.align() == 0);
        trace!("heap range = {:?}", heap_range);
        let brk = AsyncRwLock::new(heap_range.start());
        chunks.insert(chunk_ref);

        // Init the stack memory in the process
        let stack_layout = &other_layouts[1];
        let vm_option = {
            let vm_option = VMMapOptionsBuilder::default()
                .size(stack_layout.size())
                .align(heap_layout.align())
                .perms(VMPerms::READ | VMPerms::WRITE)
                .build();
            if vm_option.is_err() {
                self.handle_error_when_init(&chunks).await
            }
            vm_option?
        };
        let (stack_range, chunk_ref) = {
            let res = USER_SPACE_VM_MANAGER.alloc(&vm_option).await;
            if res.is_err() {
                self.handle_error_when_init(&chunks).await
            }
            res?
        };
        debug_assert!(stack_range.start() % stack_layout.align() == 0);
        chunks.insert(chunk_ref);
        trace!("stack range = {:?}", stack_range);

        let mem_chunks = Arc::new(AsyncRwLock::new(chunks));
        Ok(ProcessVM {
            elf_ranges,
            heap_range,
            stack_range,
            brk,
            mem_chunks,
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
        Ok(())
    }

    async fn handle_error_when_init(&self, chunks: &HashSet<Arc<Chunk>>) {
        for chunk in chunks.iter() {
            USER_SPACE_VM_MANAGER
                .internal()
                .await
                .munmap_chunk(chunk, None)
                .await;
        }
    }

    async fn init_elf_memory(elf_range: &VMRange, elf_file: &ElfFile<'a>) -> Result<()> {
        // Destination buffer: ELF appeared in the process
        let elf_proc_buf = unsafe { elf_range.as_slice_mut() };
        // Source buffer: ELF stored in the ELF file
        let elf_file_buf = elf_file.as_slice();

        let base_load_address_offset = elf_file.base_load_address_offset() as usize;

        // Offsets to track zerolized range
        let mut empty_start_offset = 0;
        let mut empty_end_offset = 0;

        // Init all loadable segments
        let elf_file_handle = elf_file
            .file_ref()
            .as_async_file_handle()
            .ok_or_else(|| errno!(EINVAL, "not file handle"))?;
        if !elf_file_handle.access_mode().readable() {
            return_errno!(EBADF, "elf file is not readable");
        }
        let elf_file_inode = elf_file_handle.dentry().inode();
        for segment in elf_file
            .program_headers()
            .filter(|segment| segment.loadable())
        {
            let file_size = segment.p_filesz as usize;
            let file_offset = segment.p_offset as usize;
            let mem_addr = segment.p_vaddr as usize;
            let mem_size = segment.p_memsz as usize;
            let alignment = segment.p_align as usize;
            debug_assert!(file_size <= mem_size);

            let mem_start_offset = mem_addr - base_load_address_offset;

            // Initialize empty part to zero based on alignment
            empty_start_offset = align_down(mem_start_offset, alignment);
            for b in &mut elf_proc_buf[empty_start_offset..mem_start_offset] {
                *b = 0;
            }

            // Bytes of file_size length are loaded from the ELF file
            elf_file_inode
                .read_at(
                    file_offset,
                    &mut elf_proc_buf[mem_start_offset..mem_start_offset + file_size],
                )
                .await?;

            // Set the remaining part to zero based on alignment
            debug_assert!(file_size <= mem_size);
            empty_end_offset = align_up(mem_start_offset + mem_size, alignment);
            for b in &mut elf_proc_buf[mem_start_offset + file_size..empty_end_offset] {
                *b = 0;
            }
        }

        Ok(())
    }
}

// MemChunks is the structure to track all the chunks which are used by this process.
type MemChunks = Arc<AsyncRwLock<HashSet<ChunkRef>>>;

/// The per-process virtual memory
#[derive(Debug)]
pub struct ProcessVM {
    elf_ranges: Vec<VMRange>,
    heap_range: VMRange,
    stack_range: VMRange,
    brk: AsyncRwLock<usize>,
    // Memory safety notes: the mem_chunks field must be the last one.
    //
    // Rust drops fields in the same order as they are declared. So by making
    // mem_chunks the last field, we ensure that when all other fields are
    // dropped, their drop methods (if provided) can still access the memory
    // region represented by the mem_chunks field.
    mem_chunks: MemChunks,
}

impl Default for ProcessVM {
    fn default() -> ProcessVM {
        ProcessVM {
            elf_ranges: Default::default(),
            heap_range: Default::default(),
            stack_range: Default::default(),
            brk: Default::default(),
            mem_chunks: Arc::new(AsyncRwLock::new(HashSet::new())),
        }
    }
}

impl Drop for ProcessVM {
    fn drop(&mut self) {
        info!("Process VM dropped");
    }
}

impl ProcessVM {
    pub fn mem_chunks(&self) -> &MemChunks {
        &self.mem_chunks
    }

    pub fn stack_range(&self) -> &VMRange {
        &self.stack_range
    }

    pub fn heap_range(&self) -> &VMRange {
        &self.heap_range
    }

    pub async fn add_mem_chunk(&self, chunk: ChunkRef) {
        let mut mem_chunks = self.mem_chunks.write().await;
        mem_chunks.insert(chunk);
    }

    pub async fn remove_mem_chunk(&self, chunk: &ChunkRef) {
        let mut mem_chunks = self.mem_chunks.write().await;
        mem_chunks.remove(chunk);
    }

    pub async fn replace_mem_chunk(&self, old_chunk: &ChunkRef, new_chunk: ChunkRef) {
        self.remove_mem_chunk(old_chunk).await;
        self.add_mem_chunk(new_chunk).await
    }

    // During creating process stage, process VM is ready but there are some other errors when creating the process,
    // e.g. spawn_attribute is set to a wrong value
    pub async fn free_mem_chunks_when_create_error(&self) {
        let mut mem_chunks = self.mem_chunks.write().await;
        for chunk in mem_chunks.drain_filter(|chunk| chunk.is_single_vma()) {
            USER_SPACE_VM_MANAGER
                .internal()
                .await
                .munmap_chunk(&chunk, None)
                .await;
        }
    }

    // Try merging all connecting single VMAs of the process.
    // This is a very expensive operation.
    pub async fn merge_all_single_vma_chunks(
        mem_chunks: &mut AsyncRwLockWriteGuard<'_, HashSet<ChunkRef>>,
    ) -> Result<Vec<VMArea>> {
        // Get all single VMA chunks
        let mut single_vma_chunks = mem_chunks
            .drain_filter(|chunk| chunk.is_single_vma())
            .collect::<Vec<ChunkRef>>();
        single_vma_chunks.sort_unstable_by(|chunk_a, chunk_b| {
            chunk_a
                .range()
                .start()
                .partial_cmp(&chunk_b.range().start())
                .unwrap()
        });

        // Try merging connecting single VMA chunks
        for chunks in single_vma_chunks.windows(2) {
            let chunk_a = &chunks[0];
            let chunk_b = &chunks[1];
            let mut vma_a = match chunk_a.internal() {
                ChunkType::MultiVMA(_) => {
                    unreachable!();
                }
                ChunkType::SingleVMA(vma) => vma.lock().await,
            };

            let mut vma_b = match chunk_b.internal() {
                ChunkType::MultiVMA(_) => {
                    unreachable!();
                }
                ChunkType::SingleVMA(vma) => vma.lock().await,
            };

            if VMArea::can_merge_vmas(&vma_a, &vma_b) {
                let new_start = vma_a.start();
                vma_b.set_start(new_start);
                // set vma_a to zero
                vma_a.set_end(new_start);
            }
        }

        // Collect merged vmas which will be the output of this function
        let mut merged_vmas = Vec::new();

        // Insert unchanged chunks back to mem_chunks list and collect merged vmas for output
        for chunk in single_vma_chunks.into_iter() {
            if !chunk.is_single_dummy_vma().await {
                if chunk.is_single_vma_with_conflict_size().await {
                    let new_vma = chunk.get_vma_for_single_vma_chunk().await;
                    merged_vmas.push(new_vma);

                    // Don't insert the merged chunks to mem_chunk list here. It should be updated later.
                } else {
                    mem_chunks.insert(chunk);
                }
            }
        }

        Ok(merged_vmas)
    }

    pub fn get_process_range(&self) -> &VMRange {
        USER_SPACE_VM_MANAGER.range()
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

    pub async fn get_brk(&self) -> usize {
        *self.brk.read().await
    }

    pub async fn brk(&self, brk: usize) -> Result<usize> {
        let heap_start = self.heap_range.start();
        let heap_end = self.heap_range.end();

        // Acquire lock first to avoid data-race.
        let mut brk_guard = self.brk.write().await;

        if brk >= heap_start && brk <= heap_end {
            // Get page-aligned brk address.
            let new_brk = align_up(brk, PAGE_SIZE);
            // Get page-aligned old brk address.
            let old_brk = align_up(*brk_guard, PAGE_SIZE);

            // Reset the memory when brk shrinks.
            if new_brk < old_brk {
                let shrink_brk_range =
                    VMRange::new(new_brk, old_brk).expect("shrink brk range must be valid");
                USER_SPACE_VM_MANAGER.reset_memory(shrink_brk_range).await?;
            }

            // Return the user-specified brk address without page aligned. This is same as Linux.
            *brk_guard = brk;
            Ok(brk)
        } else {
            if brk < heap_start {
                error!("New brk address is too low");
            } else if brk > heap_end {
                error!("New brk address is too high");
            }

            Ok(*brk_guard)
        }
    }

    // Get a NON-accurate free size for current process
    pub async fn get_free_size(&self) -> usize {
        let chunk_free_size = {
            let mut chunk_free_size = 0;
            let process_chunks = self.mem_chunks.read().await;
            for chunk in process_chunks.iter() {
                chunk_free_size += chunk.free_size().await;
            }
            chunk_free_size
        };
        let free_size = chunk_free_size + USER_SPACE_VM_MANAGER.free_size().await;
        free_size
    }

    pub async fn mmap(
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
                // There is no need to fill zeros in mmap. Cleaning is done after munmap.
                VMInitializer::DoNothing()
            } else {
                let file_ref = current!().file(fd)?;
                // Only shared, file-backed memory mappings have write-back files
                let need_write_back = if flags.contains(MMapFlags::MAP_SHARED) {
                    true
                } else {
                    false
                };
                VMInitializer::FileBacked {
                    file: FileBacked::new(file_ref, offset, need_write_back),
                }
            }
        };
        let mmap_options = VMMapOptionsBuilder::default()
            .size(size)
            .addr(addr_option)
            .perms(perms)
            .initializer(initializer)
            .build()?;
        let mmap_addr = USER_SPACE_VM_MANAGER.mmap(&mmap_options).await?;
        Ok(mmap_addr)
    }

    pub async fn mremap(
        &self,
        old_addr: usize,
        old_size: usize,
        new_size: usize,
        flags: MRemapFlags,
    ) -> Result<usize> {
        let mremap_option = VMRemapOptions::new(old_addr, old_size, new_size, flags)?;
        USER_SPACE_VM_MANAGER.mremap(&mremap_option).await
    }

    pub async fn munmap(&self, addr: usize, size: usize) -> Result<()> {
        USER_SPACE_VM_MANAGER.munmap(addr, size).await
    }

    pub async fn mprotect(&self, addr: usize, size: usize, perms: VMPerms) -> Result<()> {
        let size = {
            if size == 0 {
                return Ok(());
            }
            align_up(size, PAGE_SIZE)
        };
        let protect_range = VMRange::new_with_size(addr, size)?;

        return USER_SPACE_VM_MANAGER.mprotect(addr, size, perms).await;
    }

    pub async fn msync(&self, addr: usize, size: usize) -> Result<()> {
        return USER_SPACE_VM_MANAGER.msync(addr, size).await;
    }

    pub async fn msync_by_file(&self, sync_file: &FileRef) {
        return USER_SPACE_VM_MANAGER.msync_by_file(sync_file).await;
    }

    // Return: a copy of the found region
    pub async fn find_mmap_region(&self, addr: usize) -> Result<VMRange> {
        USER_SPACE_VM_MANAGER.find_mmap_region(addr).await
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

// TODO: Support MREMAP_DONTUNMAP flag (since Linux 5.7)
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
