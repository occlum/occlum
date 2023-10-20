//! Shared memory manager. (POSIX)
use super::*;

use super::vm_manager::{InternalVMManager, MunmapChunkFlag};
use super::vm_util::VMMapOptions;
use crate::process::ThreadStatus;

use rcore_fs::vfs::{FileType, Metadata};
use std::collections::HashMap;
use std::sync::{Arc, Weak};

type InodeId = usize;

/// Shared VM manager.
#[derive(Debug)]
pub struct ShmManager {
    // K: Inode id of shared backed file. V: Chunk which is shared by processes.
    shared_chunks: HashMap<InodeId, ChunkRef>,
}

/// Result types of `mmap()` with `MAP_SHARED`.
#[derive(Clone, Debug)]
pub enum MmapSharedResult {
    /// Can share successfully
    Success(usize),
    /// Need to create a new shared chunk
    NeedCreate,
    /// Current shared chunk needs to expand range. (old shared chunk, expand range)
    NeedExpand(ChunkRef, VMRange),
    /// Current shared chunk needs to be replaced to satisfy new request
    NeedReplace(ChunkRef),
}

/// Result types of unmapping a shared memory chunk.
/// This could come from `munmap()` or `mremap` request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MunmapSharedResult {
    /// Current shared chunk is still being shared and used by other processes
    StillInUse,
    /// All shared processes are detached, the chunk can be freed
    Freeable,
}

impl ShmManager {
    pub fn new() -> Self {
        Self {
            shared_chunks: HashMap::new(),
        }
    }

    pub fn mmap_shared_chunk(&mut self, options: &VMMapOptions) -> Result<MmapSharedResult> {
        Self::qualified_for_sharing(options)?;

        let backed_file = options.initializer().backed_file().unwrap();
        let inode_id = backed_file.metadata().inode;
        let offset = backed_file.offset();

        let shared_chunk = match self.shared_chunks.get(&inode_id) {
            Some(shared_chunk) => shared_chunk,
            None => {
                return Ok(MmapSharedResult::NeedCreate);
            }
        };
        let mut shared_vma = Self::vma_of(&shared_chunk);

        let current = current!();
        let current_pid = current.process().pid();

        let contained = shared_vma.belong_to(current_pid);
        let exclusived = shared_vma.exclusive_by(current_pid);

        let addr = {
            let sc_addr = shared_vma.start();
            let sc_size = shared_vma.size();
            let sc_offset = shared_vma
                .writeback_file()
                .map(|(_, offset)| offset)
                .unwrap();
            let new_size = *options.size();
            let mut target_addr = usize::MAX;
            match *options.addr() {
                vm_util::VMMapAddr::Any | vm_util::VMMapAddr::Hint(_) => {
                    if offset == sc_offset && new_size <= sc_size {
                        target_addr = sc_addr;
                    } else if offset > sc_offset && offset - sc_offset + new_size <= sc_size {
                        target_addr = sc_addr + offset - sc_offset;
                    } else if exclusived {
                        return Ok(MmapSharedResult::NeedReplace(shared_chunk.clone()));
                    } else {
                        return_errno!(EINVAL, "mmap shared chunk failed");
                    }
                }
                vm_util::VMMapAddr::Need(addr) | vm_util::VMMapAddr::Force(addr) => {
                    if addr == sc_addr && offset == sc_offset && new_size <= sc_size {
                        target_addr = addr;
                    } else if Self::can_expand_shared_vma(
                        &shared_vma,
                        (
                            &VMRange::new_with_size(addr, new_size)?,
                            options
                                .initializer()
                                .backed_file()
                                .unwrap()
                                .writeback_file()
                                .unwrap(),
                            options.perms(),
                        ),
                    ) {
                        return Ok(MmapSharedResult::NeedExpand(
                            shared_chunk.clone(),
                            VMRange::new_with_size(addr, new_size).unwrap(),
                        ));
                    } else if exclusived {
                        return Ok(MmapSharedResult::NeedReplace(shared_chunk.clone()));
                    } else {
                        return_errno!(EINVAL, "mmap shared chunk failed");
                    }
                }
            }
            target_addr
        };

        Self::apply_new_perms_if_higher(&mut shared_vma, *options.perms());
        shared_vma.attach_shared_process(current_pid)?;
        if !contained {
            current.vm().add_mem_chunk(shared_chunk.clone());
        }

        Ok(MmapSharedResult::Success(addr))
    }

    pub fn munmap_shared_chunk(
        &mut self,
        chunk: &ChunkRef,
        unmap_range: &VMRange,
        flag: MunmapChunkFlag,
    ) -> Result<MunmapSharedResult> {
        debug_assert!(chunk.is_shared());
        let mut shared_vma = Self::vma_of(chunk);
        let shared_range = shared_vma.range();
        let current_pid = current!().process().pid();
        let partial_unmap = !unmap_range.is_superset_of(shared_range);

        // Fails when force unmap a partial of shared chunk which is still shared by other process
        if flag == MunmapChunkFlag::Force
            && (partial_unmap || !shared_vma.exclusive_by(current_pid))
        {
            return_errno!(EINVAL, "force unmap shared chunk failed");
        }

        // Treat partial unmapped shared chunk as still-in-use(do nothing)
        if partial_unmap {
            return Ok(MunmapSharedResult::StillInUse);
        }

        let force_detach = match flag {
            MunmapChunkFlag::Default => false,
            MunmapChunkFlag::Force | MunmapChunkFlag::OnProcessExit => true,
        };
        if shared_vma.detach_shared_process(current_pid, force_detach)? {
            self.shared_chunks.remove(&Self::inode_id_of(&shared_vma));
            Ok(MunmapSharedResult::Freeable)
        } else {
            Ok(MunmapSharedResult::StillInUse)
        }
    }

    pub fn mprotect_shared_chunk(&self, chunk: &ChunkRef, new_perms: VMPerms) -> Result<()> {
        let mut vma = Self::vma_of(chunk);
        if !vma.is_shared() {
            return_errno!(EINVAL, "not a shared chunk");
        }
        if let Some((file_ref, _)) = vma.writeback_file() {
            if !file_ref.access_mode().unwrap().writable() && new_perms.can_write() {
                return_errno!(EACCES, "file is not writable");
            }
        }
        Self::apply_new_perms_if_higher(&mut vma, new_perms);
        Ok(())
    }

    pub fn create_shared_chunk(
        &mut self,
        options: &VMMapOptions,
        new_chunk: ChunkRef,
    ) -> Result<usize> {
        let backed_file = options.initializer().backed_file().ok_or(errno!(EINVAL))?;
        let (inode_id, addr) = {
            let mut new_vma = Self::vma_of(&new_chunk);
            new_vma.mark_shared();

            let inode_id = backed_file.metadata().inode;
            debug_assert_eq!(inode_id, Self::inode_id_of(&new_vma));
            (inode_id, new_vma.start())
        };

        self.shared_chunks.insert(inode_id, new_chunk);
        Ok(addr)
    }

    // Replace the given old shared chunk with a new one. The new shared chunk would inherit
    // the access and perms from the old one.
    pub fn replace_shared_chunk(&mut self, old_shared_chunk: ChunkRef, new_chunk: ChunkRef) {
        debug_assert!(old_shared_chunk.is_shared());
        let inode_id = {
            let new_vma = Self::vma_of(&new_chunk);
            let old_vma = Self::vma_of(&old_shared_chunk);

            let inode_id = Self::inode_id_of(&new_vma);
            debug_assert_eq!(inode_id, Self::inode_id_of(&old_vma));
            inode_id
        };

        let replaced = self.shared_chunks.insert(inode_id, new_chunk).unwrap();
        debug_assert!(Arc::ptr_eq(&replaced, &old_shared_chunk));
    }

    // Left: Old shared vma. Right: New vm range, backed file and offset, perms.
    fn can_expand_shared_vma(lhs: &VMArea, rhs: (&VMRange, (&FileRef, usize), &VMPerms)) -> bool {
        debug_assert!(lhs.is_shared());
        let (lhs_range, lhs_file, lhs_file_offset, lhs_perms) = {
            let writeback_file = lhs.writeback_file().unwrap();
            (lhs.range(), writeback_file.0, writeback_file.1, lhs.perms())
        };
        let (rhs_range, (rhs_file, rhs_file_offset), rhs_perms) = rhs;

        // The two vm ranges must not be empty, and must be border with each other
        if lhs_range.size() == 0 || rhs_range.size() == 0 {
            return false;
        }
        if lhs_range.end() != rhs_range.start() {
            return false;
        }

        // The two vm must have consistent perms
        if lhs_perms != *rhs_perms {
            return false;
        }

        // The two vm must have the same backed file and consecutive offset
        // within one process
        Arc::ptr_eq(lhs_file, rhs_file)
            && rhs_file_offset > lhs_file_offset
            && rhs_file_offset - lhs_file_offset == lhs_range.size()
    }

    fn qualified_for_sharing(options: &VMMapOptions) -> Result<()> {
        if !options.is_shared() {
            return_errno!(EINVAL, "not a mmap(MAP_SHARED) request");
        }
        let backed_file = options.initializer().backed_file().unwrap();
        if backed_file.metadata().type_ != FileType::File {
            return_errno!(
                EINVAL,
                "unsupported file type when creating shared mappings"
            );
        }
        Ok(())
    }

    fn vma_of(chunk: &ChunkRef) -> SgxMutexGuard<VMArea> {
        match chunk.internal() {
            ChunkType::SingleVMA(vma) => vma.lock().unwrap(),
            ChunkType::MultiVMA(_) => unreachable!(),
        }
    }

    /// Associated functions below only applied to shared vmas.

    fn inode_id_of(vma: &SgxMutexGuard<VMArea>) -> InodeId {
        debug_assert!(vma.is_shared());
        vma.writeback_file()
            .map(|(file, _)| file.metadata().unwrap().inode)
            .unwrap()
    }

    fn apply_new_perms_if_higher(vma: &mut SgxMutexGuard<VMArea>, new_perms: VMPerms) {
        debug_assert!(vma.is_shared());
        let old_perms = vma.perms();
        let perms = new_perms | old_perms;
        if perms == old_perms {
            return;
        }
        vma.set_perms(perms);
        vma.modify_permissions_for_committed_pages(old_perms, perms);
    }
}
