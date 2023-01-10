use super::*;

use super::chunk::{
    Chunk, ChunkID, ChunkRef, ChunkType, CHUNK_DEFAULT_SIZE, DUMMY_CHUNK_PROCESS_ID,
};
use super::free_space_manager::VMFreeSpaceManager;
use super::vm_area::VMArea;
use super::vm_chunk_manager::ChunkManager;
use super::vm_perms::VMPerms;
use super::vm_util::*;
use crate::process::{ThreadRef, ThreadStatus};
use std::ops::Bound::{Excluded, Included};

use crate::util::sync::rw_lock;
use std::collections::{BTreeSet, HashSet};

// Incorrect order of locks could cause deadlock easily.
// Don't hold a low-order lock and then try to get a high-order lock.
// High order -> Low order:
// VMManager.internal > ProcessVM.mem_chunks > locks in chunks

#[derive(Debug)]
pub struct VMManager {
    range: VMRange,
    internal: SgxMutex<InternalVMManager>,
}

impl VMManager {
    pub fn init(vm_range: VMRange) -> Result<Self> {
        let internal = InternalVMManager::init(vm_range.clone());
        Ok(VMManager {
            range: vm_range,
            internal: SgxMutex::new(internal),
        })
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn internal(&self) -> SgxMutexGuard<InternalVMManager> {
        self.internal.lock().unwrap()
    }

    pub fn free_size(&self) -> usize {
        self.internal().free_manager.free_size()
    }

    pub fn verified_clean_when_exit(&self) -> bool {
        let internal = self.internal();
        internal.chunks.len() == 0 && internal.free_manager.free_size() == self.range.size()
    }

    pub fn free_chunk(&self, chunk: &ChunkRef) {
        let mut internal = self.internal();
        internal.free_chunk(chunk);
    }

    // Allocate single VMA chunk for new process whose process VM is not ready yet
    pub fn alloc(&self, options: &VMMapOptions) -> Result<(VMRange, ChunkRef)> {
        if let Ok(new_chunk) = self.internal().mmap_chunk(options) {
            return Ok((new_chunk.range().clone(), new_chunk));
        }
        return_errno!(ENOMEM, "can't allocate free chunks");
    }

    pub fn mmap(&self, options: &VMMapOptions) -> Result<usize> {
        let addr = *options.addr();
        let size = *options.size();
        let align = *options.align();

        match addr {
            VMMapAddr::Any => {}
            VMMapAddr::Hint(addr) => {
                let target_range = VMRange::new_with_size(addr, size)?;
                let ret = self.internal().mmap_with_addr(target_range, options);
                if ret.is_ok() {
                    return ret;
                }
            }
            VMMapAddr::Need(addr) | VMMapAddr::Force(addr) => {
                let target_range = VMRange::new_with_size(addr, size)?;
                return self.internal().mmap_with_addr(target_range, options);
            }
        }

        if size > CHUNK_DEFAULT_SIZE {
            if let Ok(new_chunk) = self.internal().mmap_chunk(options) {
                let start = new_chunk.range().start();
                current!().vm().add_mem_chunk(new_chunk);
                return Ok(start);
            } else {
                return_errno!(ENOMEM, "can't allocate free chunks");
            }
        }

        // Allocate in default chunk
        let current = current!();
        {
            // Fast path: Try to go to assigned chunks to do mmap
            // There is no lock on VMManager in this path.
            let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
            for chunk in process_mem_chunks
                .iter()
                .filter(|&chunk| !chunk.is_single_vma())
            {
                let result_start = chunk.try_mmap(options);
                if result_start.is_ok() {
                    return result_start;
                }
            }
        }

        // Process' chunks are all busy or can't allocate from process_mem_chunks list.
        // Allocate a new chunk with chunk default size.
        // Lock on ChunkManager.
        if let Ok(new_chunk) = self.internal().mmap_chunk_default(addr) {
            // Add this new chunk to process' chunk list
            new_chunk.add_process(&current);
            current.vm().add_mem_chunk(new_chunk.clone());

            // Allocate in the new chunk
            // This mmap could still fail due to invalid options
            return new_chunk.mmap(options);
        }

        // Slow path: Sadly, there is no free chunk, iterate every chunk to find a range
        {
            // Release lock after this block
            let mut result_start = Ok(0);
            let chunks = &self.internal().chunks;
            let chunk = chunks
                .iter()
                .filter(|&chunk| !chunk.is_single_vma())
                .find(|&chunk| {
                    result_start = chunk.mmap(options);
                    result_start.is_ok()
                });
            if let Some(chunk) = chunk {
                chunk.add_process(&current);
                current.vm().add_mem_chunk(chunk.clone());
                return result_start;
            }
        }

        // Can't find a range in default chunks. Maybe there is still free range in the global free list.
        if let Ok(new_chunk) = self.internal().mmap_chunk(options) {
            let start = new_chunk.range().start();
            current!().vm().add_mem_chunk(new_chunk);
            return Ok(start);
        }

        // No free range
        return_errno!(ENOMEM, "Can't find a free chunk for this allocation");
    }

    pub fn munmap(&self, addr: usize, size: usize) -> Result<()> {
        // Go to every process chunk to see if it contains the range.
        let size = {
            if size == 0 {
                return_errno!(EINVAL, "size of munmap must not be zero");
            }
            align_up(size, PAGE_SIZE)
        };
        let munmap_range = { VMRange::new_with_size(addr, size) }?;
        let current = current!();
        let chunk = {
            let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
            let chunk = process_mem_chunks
                .iter()
                .find(|&chunk| chunk.range().intersect(&munmap_range).is_some());
            if chunk.is_none() {
                // Note:
                // The man page of munmap states that "it is not an error if the indicated
                // range does not contain any mapped pages". This is not considered as
                // an error!
                trace!("the munmap range is not mapped");
                return Ok(());
            }
            chunk.unwrap().clone()
        };

        // Case 1: the overlapping chunk IS NOT a super set of munmap range
        if !chunk.range().is_superset_of(&munmap_range) {
            // munmap range spans multiple chunks

            // Must lock the internal manager first here in case the chunk's range and vma are conflict when other threads are operating the VM
            let mut internal_manager = self.internal();
            let overlapping_chunks = {
                let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
                process_mem_chunks
                    .iter()
                    .filter(|p_chunk| p_chunk.range().overlap_with(&munmap_range))
                    .cloned()
                    .collect::<Vec<ChunkRef>>()
            };

            for chunk in overlapping_chunks.iter() {
                match chunk.internal() {
                    ChunkType::SingleVMA(_) => {
                        internal_manager.munmap_chunk(chunk, Some(&munmap_range))?
                    }
                    ChunkType::MultiVMA(manager) => manager
                        .lock()
                        .unwrap()
                        .chunk_manager_mut()
                        .munmap_range(munmap_range)?,
                }
            }
            return Ok(());
        }

        // Case 2: the overlapping chunk IS a super set of munmap range
        debug_assert!(chunk.range().is_superset_of(&munmap_range));
        match chunk.internal() {
            ChunkType::MultiVMA(manager) => {
                return manager
                    .lock()
                    .unwrap()
                    .chunk_manager_mut()
                    .munmap_range(munmap_range);
            }
            ChunkType::SingleVMA(_) => {
                // Single VMA Chunk could be updated during the release of internal manager lock. Get overlapping chunk again.
                // This is done here because we don't want to acquire the big internal manager lock as soon as entering this function.
                let mut internal_manager = self.internal();
                let overlapping_chunk = {
                    let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
                    process_mem_chunks
                        .iter()
                        .find(|&chunk| chunk.range().intersect(&munmap_range).is_some())
                        .map(|chunk| chunk.clone())
                };
                if let Some(overlapping_chunk) = overlapping_chunk {
                    return internal_manager.munmap_chunk(&overlapping_chunk, Some(&munmap_range));
                } else {
                    warn!("no overlapping chunks anymore");
                    return Ok(());
                }
            }
        }
    }

    pub fn find_mmap_region(&self, addr: usize) -> Result<VMRange> {
        let current = current!();
        let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
        let mut vm_range = Ok(Default::default());
        process_mem_chunks.iter().find(|&chunk| {
            vm_range = chunk.find_mmap_region(addr);
            vm_range.is_ok()
        });
        return vm_range;
    }

    pub fn mprotect(&self, addr: usize, size: usize, perms: VMPerms) -> Result<()> {
        let protect_range = VMRange::new_with_size(addr, size)?;
        let chunk = {
            let current = current!();
            let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
            let chunk = process_mem_chunks
                .iter()
                .find(|&chunk| chunk.range().intersect(&protect_range).is_some());
            if chunk.is_none() {
                return_errno!(ENOMEM, "invalid range");
            }
            chunk.unwrap().clone()
        };

        // TODO: Support mprotect range spans multiple chunks
        if !chunk.range().is_superset_of(&protect_range) {
            return_errno!(EINVAL, "mprotect range is not in a single chunk");
        }

        match chunk.internal() {
            ChunkType::MultiVMA(manager) => {
                trace!("mprotect default chunk: {:?}", chunk.range());
                return manager
                    .lock()
                    .unwrap()
                    .chunk_manager_mut()
                    .mprotect(addr, size, perms);
            }
            ChunkType::SingleVMA(_) => {
                let mut internal_manager = self.internal();
                return internal_manager.mprotect_single_vma_chunk(&chunk, protect_range, perms);
            }
        }
    }

    // Reset memory permission to default (R/W) and reset the memory contents to zero. Currently only used by brk.
    pub fn reset_memory(&self, reset_range: VMRange) -> Result<()> {
        let intersect_chunks = {
            let chunks = self
                .internal()
                .chunks
                .iter()
                .filter(|&chunk| chunk.range().intersect(&reset_range).is_some())
                .map(|chunk| chunk.clone())
                .collect::<Vec<_>>();

            // In the heap area, there shouldn't be any default chunks or chunks owned by other process.
            if chunks
                .iter()
                .any(|chunk| !chunk.is_owned_by_current_process() || !chunk.is_single_vma())
            {
                return_errno!(EINVAL, "There is something wrong with the intersect chunks");
            }
            chunks
        };

        intersect_chunks.iter().for_each(|chunk| {
            if let ChunkType::SingleVMA(vma) = chunk.internal() {
                if let Some(intersection_range) = chunk.range().intersect(&reset_range) {
                    let mut internal_manager = self.internal();
                    internal_manager.mprotect_single_vma_chunk(
                        &chunk,
                        intersection_range,
                        VMPerms::DEFAULT,
                    );

                    unsafe {
                        let buf = intersection_range.as_slice_mut();
                        buf.iter_mut().for_each(|b| *b = 0)
                    }
                }
            }
        });

        Ok(())
    }

    pub fn msync(&self, addr: usize, size: usize) -> Result<()> {
        let sync_range = VMRange::new_with_size(addr, size)?;
        let chunk = {
            let current = current!();
            let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
            let chunk = process_mem_chunks
                .iter()
                .find(|&chunk| chunk.range().is_superset_of(&sync_range));
            if chunk.is_none() {
                return_errno!(ENOMEM, "invalid range");
            }
            chunk.unwrap().clone()
        };

        match chunk.internal() {
            ChunkType::MultiVMA(manager) => {
                trace!("msync default chunk: {:?}", chunk.range());
                return manager
                    .lock()
                    .unwrap()
                    .chunk_manager_mut()
                    .msync_by_range(&sync_range);
            }
            ChunkType::SingleVMA(vma) => {
                let vma = vma.lock().unwrap();
                ChunkManager::flush_file_vma(&vma);
            }
        }
        Ok(())
    }

    pub fn msync_by_file(&self, sync_file: &FileRef) {
        let current = current!();
        let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
        let is_same_file = |file: &FileRef| -> bool { Arc::ptr_eq(&file, &sync_file) };
        process_mem_chunks
            .iter()
            .for_each(|chunk| match chunk.internal() {
                ChunkType::MultiVMA(manager) => {
                    manager
                        .lock()
                        .unwrap()
                        .chunk_manager_mut()
                        .msync_by_file(sync_file);
                }
                ChunkType::SingleVMA(vma) => {
                    ChunkManager::flush_file_vma_with_cond(&vma.lock().unwrap(), is_same_file);
                }
            });
    }

    pub fn mremap(&self, options: &VMRemapOptions) -> Result<usize> {
        let old_addr = options.old_addr();
        let old_size = options.old_size();
        let old_range = VMRange::new_with_size(old_addr, old_size)?;
        let new_size = options.new_size();
        let size_type = VMRemapSizeType::new(&old_size, &new_size);
        let current = current!();

        // Try merging all connecting chunks
        {
            // Must lock the internal manager first here in case the chunk's range and vma are conflict when other threads are operating the VM
            let mut internal_manager = self.internal.lock().unwrap();
            let mut merged_vmas = current.vm().merge_all_single_vma_chunks()?;
            internal_manager.clean_single_vma_chunks();
            while merged_vmas.len() != 0 {
                let merged_vma = merged_vmas.pop().unwrap();
                internal_manager.add_new_chunk(&current, merged_vma);
            }
        }

        // Deternmine the chunk of the old range
        let chunk = {
            let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
            let chunk = process_mem_chunks
                .iter()
                .find(|&chunk| chunk.range().is_superset_of(&old_range));
            if chunk.is_none() {
                return_errno!(ENOMEM, "invalid range");
            }

            chunk.unwrap().clone()
        };

        // Parse the mremap options to mmap options and munmap options
        let remap_result_option = match chunk.internal() {
            ChunkType::MultiVMA(manager) => manager
                .lock()
                .unwrap()
                .chunk_manager_mut()
                .parse_mremap_options(options),
            ChunkType::SingleVMA(vma) => {
                self.parse_mremap_options_for_single_vma_chunk(options, vma)
            }
        }?;
        trace!("mremap options after parsing = {:?}", remap_result_option);

        let ret_addr = if let Some(mmap_options) = remap_result_option.mmap_options() {
            let mmap_addr = self.mmap(mmap_options);

            // FIXME: For MRemapFlags::MayMove flag, we checked if the prefered range is free when parsing the options.
            // But there is no lock after the checking, thus the mmap might fail. In this case, we should try mmap again.
            if mmap_addr.is_err() && remap_result_option.may_move() == true {
                return_errno!(
                    EAGAIN,
                    "There might still be a space for this mremap request"
                );
            }

            if remap_result_option.mmap_result_addr().is_none() {
                mmap_addr.unwrap()
            } else {
                remap_result_option.mmap_result_addr().unwrap()
            }
        } else {
            old_addr
        };

        if let Some((munmap_addr, munmap_size)) = remap_result_option.munmap_args() {
            self.munmap(*munmap_addr, *munmap_size)
                .expect("Shouln't fail");
        }

        return Ok(ret_addr);
    }

    fn parse_mremap_options_for_single_vma_chunk(
        &self,
        options: &VMRemapOptions,
        chunk_vma: &SgxMutex<VMArea>,
    ) -> Result<VMRemapResult> {
        let mut vm_manager = self.internal.lock().unwrap();
        let chunk_vma = chunk_vma.lock().unwrap();
        vm_manager.parse(options, &chunk_vma)
    }

    // When process is exiting, free all owned chunks
    pub fn free_chunks_when_exit(&self, thread: &ThreadRef) {
        let mut internal_manager = self.internal();
        let mut mem_chunks = thread.vm().mem_chunks().write().unwrap();

        mem_chunks.iter().for_each(|chunk| {
            internal_manager.munmap_chunk(&chunk, None);
        });
        mem_chunks.clear();

        assert!(mem_chunks.len() == 0);
    }
}

// Modification on this structure must aquire the global lock.
// TODO: Enable fast_default_chunks for faster chunk allocation
#[derive(Debug)]
pub struct InternalVMManager {
    chunks: BTreeSet<ChunkRef>, // track in-use chunks, use B-Tree for better performance and simplicity (compared with red-black tree)
    fast_default_chunks: Vec<ChunkRef>, // empty default chunks
    free_manager: VMFreeSpaceManager,
}

impl InternalVMManager {
    pub fn init(vm_range: VMRange) -> Self {
        let chunks = BTreeSet::new();
        let fast_default_chunks = Vec::new();
        let free_manager = VMFreeSpaceManager::new(vm_range);
        Self {
            chunks,
            fast_default_chunks,
            free_manager,
        }
    }

    // Allocate a new chunk with default size
    pub fn mmap_chunk_default(&mut self, addr: VMMapAddr) -> Result<ChunkRef> {
        // Find a free range from free_manager
        let free_range = self.find_free_gaps(CHUNK_DEFAULT_SIZE, PAGE_SIZE, addr)?;

        // Add this range to chunks
        let chunk = Arc::new(Chunk::new_default_chunk(free_range)?);
        trace!("allocate a default chunk = {:?}", chunk);
        self.chunks.insert(chunk.clone());
        Ok(chunk)
    }

    // Allocate a chunk with single vma
    pub fn mmap_chunk(&mut self, options: &VMMapOptions) -> Result<ChunkRef> {
        let addr = *options.addr();
        let size = *options.size();
        let align = *options.align();
        let free_range = self.find_free_gaps(size, align, addr)?;
        let free_chunk = Chunk::new_single_vma_chunk(&free_range, options);
        if let Err(e) = free_chunk {
            // Error when creating chunks. Must return the free space before returning error
            self.free_manager
                .add_range_back_to_free_manager(&free_range);
            return_errno!(e.errno(), "mmap_chunk failure");
        }
        let chunk = Arc::new(free_chunk.unwrap());
        trace!("allocate a new single vma chunk: {:?}", chunk);
        self.chunks.insert(chunk.clone());
        Ok(chunk)
    }

    // Munmap a chunk
    // For Single VMA chunk, a part of the chunk could be munmapped if munmap_range is specified.
    pub fn munmap_chunk(&mut self, chunk: &ChunkRef, munmap_range: Option<&VMRange>) -> Result<()> {
        trace!(
            "munmap_chunk range = {:?}, munmap_range = {:?}",
            chunk.range(),
            munmap_range
        );
        let vma = match chunk.internal() {
            ChunkType::MultiVMA(manager) => {
                let mut manager = manager.lock().unwrap();
                let is_cleaned = manager.clean_multi_vmas();
                // If the manager is cleaned, there is only one process using this chunk. Thus it can be freed safely.
                // If the manager is not cleaned, there is at least another process which is using this chunk. Don't free it here.
                if is_cleaned {
                    self.free_chunk(chunk)?;
                }
                return Ok(());
            }
            ChunkType::SingleVMA(vma) => vma,
        };

        let munmap_range = {
            if munmap_range.is_none() {
                chunk.range()
            } else {
                munmap_range.unwrap()
            }
        };

        // Either the munmap range is a subset of the chunk range or the munmap range is
        // a superset of the chunk range. We can handle both cases.

        let mut vma = vma.lock().unwrap();
        debug_assert!(chunk.range() == vma.range());
        let intersection_vma = match vma.intersect(munmap_range) {
            Some(intersection_vma) => intersection_vma,
            _ => unreachable!(),
        };

        // File-backed VMA needs to be flushed upon munmap
        ChunkManager::flush_file_vma(&intersection_vma);

        // Reset memory permissions
        if !&intersection_vma.perms().is_default() {
            VMPerms::apply_perms(&intersection_vma, VMPerms::default());
        }

        // Reset to zero
        unsafe {
            let buf = intersection_vma.as_slice_mut();
            buf.iter_mut().for_each(|b| *b = 0)
        }

        let mut new_vmas = vma.subtract(&intersection_vma);
        let current = current!();
        // Release lock in chunk before getting lock for process mem_chunks to avoid deadlock
        drop(vma);

        match new_vmas.len() {
            0 => {
                // Exact size
                self.free_chunk(&chunk);
                if current.status() != ThreadStatus::Exited {
                    // If the current thread is exiting, there is no need to remove the chunk from process' mem_list.
                    // It will be drained.
                    current.vm().remove_mem_chunk(&chunk);
                }
            }
            1 => {
                // Update the current vma to the new vma
                let updated_vma = new_vmas.pop().unwrap();
                self.update_single_vma_chunk(&current, &chunk, updated_vma);

                // Return the intersection range to free list
                self.free_manager
                    .add_range_back_to_free_manager(intersection_vma.range());
            }
            2 => {
                // single vma => (updated_vma, munmapped_vma, new_vma)
                self.free_manager
                    .add_range_back_to_free_manager(intersection_vma.range());

                let new_vma = new_vmas.pop().unwrap();
                let new_vma_chunk = Arc::new(Chunk::new_chunk_with_vma(new_vma));
                self.chunks.insert(new_vma_chunk.clone());
                current.vm().add_mem_chunk(new_vma_chunk);

                let updated_vma = new_vmas.pop().unwrap();
                self.update_single_vma_chunk(&current, &chunk, updated_vma);
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn update_single_vma_chunk(
        &mut self,
        current_thread: &ThreadRef,
        old_chunk: &ChunkRef,
        new_vma: VMArea,
    ) {
        let new_chunk = Arc::new(Chunk::new_chunk_with_vma(new_vma));
        current_thread
            .vm()
            .replace_mem_chunk(old_chunk, new_chunk.clone());
        self.chunks.remove(old_chunk);
        self.chunks.insert(new_chunk);
    }

    // protect_range should a sub-range of the chunk range
    pub fn mprotect_single_vma_chunk(
        &mut self,
        chunk: &ChunkRef,
        protect_range: VMRange,
        new_perms: VMPerms,
    ) -> Result<()> {
        debug_assert!(chunk.range().is_superset_of(&protect_range));
        let vma = match chunk.internal() {
            ChunkType::MultiVMA(_) => {
                unreachable!();
            }
            ChunkType::SingleVMA(vma) => vma,
        };

        let mut updated_vmas = {
            let mut containing_vma = vma.lock().unwrap();
            trace!(
                "mprotect_single_vma_chunk range = {:?}, mprotect_range = {:?}",
                chunk.range(),
                protect_range
            );
            debug_assert!(chunk.range() == containing_vma.range());

            if containing_vma.perms() == new_perms {
                return Ok(());
            }

            let same_start = protect_range.start() == containing_vma.start();
            let same_end = protect_range.end() == containing_vma.end();
            match (same_start, same_end) {
                (true, true) => {
                    // Exact the same vma
                    containing_vma.set_perms(new_perms);
                    VMPerms::apply_perms(&containing_vma, containing_vma.perms());
                    return Ok(());
                }
                (false, false) => {
                    // The containing VMA is divided into three VMAs:
                    // Shrinked old VMA:    [containing_vma.start,     protect_range.start)
                    // New VMA:             [protect_range.start,      protect_range.end)
                    // remaining old VMA:     [protect_range.end,        containing_vma.end)

                    let old_end = containing_vma.end();
                    let old_perms = containing_vma.perms();

                    containing_vma.set_end(protect_range.start());

                    let new_vma = VMArea::inherits_file_from(
                        &containing_vma,
                        protect_range,
                        new_perms,
                        DUMMY_CHUNK_PROCESS_ID,
                    );
                    VMPerms::apply_perms(&new_vma, new_vma.perms());

                    let remaining_old_vma = {
                        let range = VMRange::new(protect_range.end(), old_end).unwrap();
                        VMArea::inherits_file_from(
                            &containing_vma,
                            range,
                            old_perms,
                            DUMMY_CHUNK_PROCESS_ID,
                        )
                    };

                    // Put containing_vma at last to be updated first.
                    let updated_vmas = vec![new_vma, remaining_old_vma, containing_vma.clone()];
                    updated_vmas
                }
                _ => {
                    if same_start {
                        // Protect range is at left side of the cotaining vma
                        containing_vma.set_start(protect_range.end());
                    } else {
                        // Protect range is at right side of the cotaining vma
                        containing_vma.set_end(protect_range.start());
                    }

                    let new_vma = VMArea::inherits_file_from(
                        &containing_vma,
                        protect_range,
                        new_perms,
                        DUMMY_CHUNK_PROCESS_ID,
                    );
                    VMPerms::apply_perms(&new_vma, new_vma.perms());

                    // Put containing_vma at last to be updated first.
                    let updated_vmas = vec![new_vma, containing_vma.clone()];
                    updated_vmas
                }
            }
        };

        let current = current!();
        // First update current vma chunk
        if updated_vmas.len() > 1 {
            let update_vma = updated_vmas.pop().unwrap();
            self.update_single_vma_chunk(&current, &chunk, update_vma);
        }

        // Then add new chunks if any
        updated_vmas.into_iter().for_each(|vma| {
            self.add_new_chunk(&current, vma);
        });

        Ok(())
    }

    // Must make sure that all the chunks are valid before adding new chunks
    fn add_new_chunk(&mut self, current_thread: &ThreadRef, new_vma: VMArea) {
        let new_vma_chunk = Arc::new(Chunk::new_chunk_with_vma(new_vma));
        let success = self.chunks.insert(new_vma_chunk.clone());
        debug_assert!(success);
        current_thread.vm().add_mem_chunk(new_vma_chunk);
    }

    pub fn free_chunk(&mut self, chunk: &ChunkRef) -> Result<()> {
        let range = chunk.range();
        // Remove from chunks
        self.chunks.remove(chunk);

        // Mprotect the whole chunk to reduce the usage of vma count of host
        VMPerms::apply_perms(range, VMPerms::DEFAULT);

        // Add range back to freespace manager
        self.free_manager.add_range_back_to_free_manager(range);
        Ok(())
    }

    pub fn find_free_gaps(
        &mut self,
        size: usize,
        align: usize,
        addr: VMMapAddr,
    ) -> Result<VMRange> {
        return self
            .free_manager
            .find_free_range_internal(size, align, addr);
    }

    pub fn clean_single_vma_chunks(&mut self) {
        self.chunks
            .drain_filter(|chunk| chunk.is_single_vma_chunk_should_be_removed())
            .collect::<BTreeSet<Arc<Chunk>>>();
    }

    // If addr is specified, use single VMA chunk to record the memory chunk.
    //
    // Previously, this method is implemented for VMManager and only acquire the internal lock when needed. However, this will make mmap non-atomic.
    // In multi-thread applications, for example, when a thread calls mmap with MAP_FIXED flag, and that desired memory is mapped already, the libos
    // will first munmap the corresponding memory and then do a mmap with desired address. If the lock is not aquired during the whole process,
    // the unmapped memory might be allocated by other thread who is waiting and acquiring the lock.
    // Thus, in current code, this method is implemented for InternalManager and holds the lock during the whole process.
    // Below method "force_mmap_across_multiple_chunks" is the same.
    fn mmap_with_addr(&mut self, target_range: VMRange, options: &VMMapOptions) -> Result<usize> {
        let addr = *options.addr();
        let size = *options.size();

        let current = current!();

        let chunk = {
            let process_mem_chunks = current.vm().mem_chunks().read().unwrap();
            process_mem_chunks
                .iter()
                .find(|&chunk| chunk.range().intersect(&target_range).is_some())
                .cloned()
        };

        if let Some(chunk) = chunk {
            // This range is currently in a allocated chunk
            match chunk.internal() {
                ChunkType::MultiVMA(chunk_internal) => {
                    if !chunk.range().is_superset_of(&target_range) && addr.is_force() {
                        // The target range spans multiple chunks and have a strong need for the address
                        return self.force_mmap_across_multiple_chunks(target_range, options);
                    }

                    trace!(
                        "mmap with addr in existing default chunk: {:?}",
                        chunk.range()
                    );
                    return chunk_internal
                        .lock()
                        .unwrap()
                        .chunk_manager_mut()
                        .mmap(options);
                }
                ChunkType::SingleVMA(_) => {
                    match addr {
                        VMMapAddr::Hint(addr) => {
                            return_errno!(ENOMEM, "Single VMA is currently in use. Hint failure");
                        }
                        VMMapAddr::Need(addr) => {
                            return_errno!(ENOMEM, "Single VMA is currently in use. Need failure");
                        }
                        VMMapAddr::Force(addr) => {
                            if !chunk.range().is_superset_of(&target_range) {
                                // The target range spans multiple chunks and have a strong need for the address
                                return self
                                    .force_mmap_across_multiple_chunks(target_range, options);
                            }

                            // Munmap the corresponding single vma chunk
                            // let mut internal_manager = self.internal();
                            self.munmap_chunk(&chunk, Some(&target_range))?;
                        }
                        VMMapAddr::Any => unreachable!(),
                    }
                }
            }
        }

        // This range is not currently using, allocate one in global list
        if let Ok(new_chunk) = self.mmap_chunk(options) {
            let start = new_chunk.range().start();
            debug_assert!({
                match addr {
                    VMMapAddr::Force(addr) | VMMapAddr::Need(addr) => start == target_range.start(),
                    _ => true,
                }
            });
            current.vm().add_mem_chunk(new_chunk);
            return Ok(start);
        } else {
            return_errno!(ENOMEM, "can't allocate a chunk in global list")
        }
    }

    fn force_mmap_across_multiple_chunks(
        &mut self,
        target_range: VMRange,
        options: &VMMapOptions,
    ) -> Result<usize> {
        match options.initializer() {
            VMInitializer::DoNothing() | VMInitializer::FillZeros() => {}
            _ => return_errno!(
                ENOSYS,
                "unsupported memory initializer in mmap across multiple chunks"
            ),
        }

        // Get all intersect chunks
        let intersect_chunks = {
            let chunks = self
                .chunks
                .iter()
                .filter(|&chunk| chunk.range().intersect(&target_range).is_some())
                .map(|chunk| chunk.clone())
                .collect::<Vec<_>>();

            // If any intersect chunk belongs to other process, then this mmap can't succeed
            if chunks
                .iter()
                .any(|chunk| !chunk.is_owned_by_current_process())
            {
                return_errno!(
                    ENOMEM,
                    "part of the target range is allocated by other process"
                );
            }
            chunks
        };

        let mut intersect_ranges = intersect_chunks
            .iter()
            .map(|chunk| chunk.range().intersect(&target_range).unwrap())
            .collect::<Vec<_>>();

        // Based on range of chunks, split the whole target range to ranges that are connected, including free ranges
        let target_contained_ranges = {
            let mut contained_ranges = Vec::with_capacity(intersect_ranges.len());
            for ranges in intersect_ranges.windows(2) {
                let range_a = ranges[0];
                let range_b = ranges[1];
                debug_assert!(range_a.end() <= range_b.start());
                contained_ranges.push(range_a);
                if range_a.end() < range_b.start() {
                    contained_ranges.push(VMRange::new(range_a.end(), range_b.start()).unwrap());
                }
            }
            contained_ranges.push(intersect_ranges.pop().unwrap());
            contained_ranges
        };

        // Based on the target contained ranges, rebuild the VMMapOptions
        let new_options = {
            let perms = options.perms().clone();
            let align = options.align().clone();
            let initializer = options.initializer();
            target_contained_ranges
                .iter()
                .map(|range| {
                    let size = range.size();
                    let addr = match options.addr() {
                        VMMapAddr::Force(_) => VMMapAddr::Force(range.start()),
                        _ => unreachable!(),
                    };

                    VMMapOptionsBuilder::default()
                        .perms(perms)
                        .align(align)
                        .initializer(initializer.clone())
                        .addr(addr)
                        .size(size)
                        .build()
                        .unwrap()
                })
                .collect::<Vec<VMMapOptions>>()
        };

        trace!(
            "force mmap across multiple chunks mmap ranges = {:?}",
            target_contained_ranges
        );
        for (range, options) in target_contained_ranges.into_iter().zip(new_options.iter()) {
            if self.mmap_with_addr(range, options).is_err() {
                // Although the error here is fatal and rare, returning error is not enough here.
                // FIXME: All previous mmap ranges should be munmapped.
                return_errno!(ENOMEM, "mmap across multiple chunks failed");
            }
        }

        Ok(target_range.start())
    }
}

impl VMRemapParser for InternalVMManager {
    fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.free_manager.is_free_range(request_range)
    }
}
