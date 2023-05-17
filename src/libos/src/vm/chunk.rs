use super::*;

use super::vm_area::VMArea;
use super::vm_chunk_manager::ChunkManager;
use super::vm_perms::VMPerms;
use super::vm_util::*;
use crate::process::ProcessRef;
use crate::process::ThreadRef;

use async_rt::sync::Mutex as AsyncMutex;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

// For single VMA chunk, the vma struct doesn't need to update the pid field. Because all the chunks are recorded by the process VM already.
pub const DUMMY_CHUNK_PROCESS_ID: pid_t = 0;
// Default chunk size: 32MB
pub const CHUNK_DEFAULT_SIZE: usize = 32 * 1024 * 1024;

pub type ChunkID = usize;
pub type ChunkRef = Arc<Chunk>;

pub struct Chunk {
    // This range is used for fast check without any locks. However, when mremap, the size of this range could be
    // different with the internal VMA range for single VMA chunk. This can only be corrected by getting the internal
    // VMA, creating a new chunk and replacing the old chunk.
    range: VMRange,
    internal: ChunkType,
}

impl Hash for Chunk {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.range.hash(state);
    }
}

impl Ord for Chunk {
    fn cmp(&self, other: &Self) -> Ordering {
        self.range.start().cmp(&other.range.start())
    }
}

impl PartialOrd for Chunk {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Chunk {
    fn eq(&self, other: &Self) -> bool {
        self.range == other.range
    }
}

impl Eq for Chunk {}

impl Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "range = {:?}, ", self.range);
        match self.internal() {
            ChunkType::SingleVMA(vma) => write!(f, "Single VMA chunk: {:?}", vma),
            ChunkType::MultiVMA(internal_manager) => write!(f, "default chunk: {:?}", self.range()),
        }
    }
}

impl Chunk {
    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn internal(&self) -> &ChunkType {
        &self.internal
    }

    pub async fn get_vma_for_single_vma_chunk(&self) -> VMArea {
        match self.internal() {
            ChunkType::MultiVMA(internal_manager) => unreachable!(),
            ChunkType::SingleVMA(vma) => return vma.lock().await.clone(),
        }
    }

    pub async fn free_size(&self) -> usize {
        match self.internal() {
            ChunkType::SingleVMA(vma) => 0, // for single VMA chunk, there is no free space
            ChunkType::MultiVMA(internal_manager) => internal_manager.lock().await.free_size(),
        }
    }

    pub fn new_default_chunk(vm_range: VMRange) -> Result<Self> {
        let internal_manager = ChunkInternal::new(vm_range)?;
        Ok(Self {
            range: vm_range,
            internal: ChunkType::MultiVMA(AsyncMutex::new(internal_manager)),
        })
    }

    pub async fn new_single_vma_chunk(vm_range: &VMRange, options: &VMMapOptions) -> Result<Self> {
        let vm_area = VMArea::new(
            vm_range.clone(),
            *options.perms(),
            options.initializer().backed_file(),
            DUMMY_CHUNK_PROCESS_ID,
        );
        // Initialize the memory of the new range
        unsafe {
            let buf = vm_range.as_slice_mut();
            options.initializer().init_slice(buf).await?;
        }
        // Set memory permissions
        if !options.perms().is_default() {
            VMPerms::apply_perms(&vm_area, vm_area.perms());
        }
        Ok(Self::new_chunk_with_vma(vm_area))
    }

    pub fn new_chunk_with_vma(vma: VMArea) -> Self {
        Self {
            range: vma.range().clone(),
            internal: ChunkType::SingleVMA(AsyncMutex::new(vma)),
        }
    }

    pub async fn is_owned_by_current_process(&self) -> bool {
        let current = current!();
        let process_mem_chunks = current.vm().mem_chunks().read().await;
        if !process_mem_chunks
            .iter()
            .any(|chunk| chunk.range() == self.range())
        {
            return false;
        }

        match self.internal() {
            ChunkType::SingleVMA(vma) => true,
            ChunkType::MultiVMA(internal_manager) => {
                let internal_manager = internal_manager.lock().await;
                internal_manager.is_owned_by_current_process()
            }
        }
    }

    pub async fn add_process(&self, current: &ThreadRef) {
        match self.internal() {
            ChunkType::SingleVMA(vma) => unreachable!(),
            ChunkType::MultiVMA(internal_manager) => {
                internal_manager
                    .lock()
                    .await
                    .add_process(current.process().pid());
            }
        }
    }

    pub async fn mmap(&self, options: &VMMapOptions) -> Result<usize> {
        debug_assert!(!self.is_single_vma());
        trace!("try allocate in chunk: {:?}", self);
        let mut internal_manager = if let ChunkType::MultiVMA(internal_manager) = &self.internal {
            internal_manager.lock().await
        } else {
            unreachable!();
        };
        if internal_manager.chunk_manager.free_size() < options.size() {
            return_errno!(ENOMEM, "no enough size without trying. try other chunks");
        }
        return internal_manager.chunk_manager.mmap(options).await;
    }

    pub async fn try_mmap(&self, options: &VMMapOptions) -> Result<usize> {
        debug_assert!(!self.is_single_vma());
        // Try lock ChunkManager. If it fails, just return and will try other chunks.
        let mut internal_manager = if let ChunkType::MultiVMA(internal_manager) = &self.internal {
            internal_manager
                .try_lock()
                .map_err(|_| errno!(EAGAIN, "try other chunks"))?
        } else {
            unreachable!();
        };
        trace!("get lock, try mmap in chunk: {:?}", self);
        if internal_manager.chunk_manager().free_size() < options.size() {
            return_errno!(ENOMEM, "no enough size without trying. try other chunks");
        }
        internal_manager.chunk_manager_mut().mmap(options).await
    }

    pub fn is_single_vma(&self) -> bool {
        if let ChunkType::SingleVMA(_) = self.internal {
            true
        } else {
            false
        }
    }

    pub async fn is_single_dummy_vma(&self) -> bool {
        if let ChunkType::SingleVMA(vma) = &self.internal {
            vma.lock().await.size() == 0
        } else {
            false
        }
    }

    // Chunk size and internal VMA size are conflict.
    // This is due to the change of internal VMA.
    pub async fn is_single_vma_with_conflict_size(&self) -> bool {
        if let ChunkType::SingleVMA(vma) = &self.internal {
            vma.lock().await.size() != self.range.size()
        } else {
            false
        }
    }

    pub async fn is_single_vma_chunk_should_be_removed(&self) -> bool {
        if let ChunkType::SingleVMA(vma) = &self.internal {
            let vma_size = vma.lock().await.size();
            vma_size == 0 || vma_size != self.range.size()
        } else {
            false
        }
    }

    pub async fn find_mmap_region(&self, addr: usize) -> Result<VMRange> {
        let internal = &self.internal;
        match self.internal() {
            ChunkType::SingleVMA(vma) => {
                let vma = vma.lock().await;
                if vma.contains(addr) {
                    return Ok(vma.range().clone());
                } else {
                    return_errno!(ESRCH, "addr not found in this chunk")
                }
            }
            ChunkType::MultiVMA(internal_manager) => {
                return internal_manager
                    .lock()
                    .await
                    .chunk_manager
                    .find_mmap_region(addr);
            }
        }
    }

    pub async fn is_free_range(&self, request_range: &VMRange) -> bool {
        match self.internal() {
            ChunkType::SingleVMA(_) => false, // single-vma chunk can't be free
            ChunkType::MultiVMA(internal_manager) => internal_manager
                .lock()
                .await
                .chunk_manager
                .is_free_range(request_range),
        }
    }
}

#[derive(Debug)]
pub enum ChunkType {
    SingleVMA(AsyncMutex<VMArea>),
    MultiVMA(AsyncMutex<ChunkInternal>),
}

#[derive(Debug)]
pub struct ChunkInternal {
    chunk_manager: ChunkManager,
    process_set: HashSet<pid_t>,
}

const PROCESS_SET_INIT_SIZE: usize = 5;

impl ChunkInternal {
    pub fn new(vm_range: VMRange) -> Result<Self> {
        let chunk_manager = ChunkManager::from(vm_range.start(), vm_range.size())?;

        let mut process_set = HashSet::with_capacity(PROCESS_SET_INIT_SIZE);
        process_set.insert(current!().process().pid());
        Ok(Self {
            chunk_manager,
            process_set,
        })
    }

    pub fn add_process(&mut self, pid: pid_t) {
        self.process_set.insert(pid);
    }

    pub fn chunk_manager(&self) -> &ChunkManager {
        &self.chunk_manager
    }

    pub fn chunk_manager_mut(&mut self) -> &mut ChunkManager {
        &mut self.chunk_manager
    }

    pub fn is_owned_by_current_process(&self) -> bool {
        let current_pid = current!().process().pid();
        self.process_set.contains(&current_pid) && self.process_set.len() == 1
    }

    pub fn free_size(&self) -> usize {
        *self.chunk_manager.free_size()
    }

    // Clean vmas when munmap a MultiVMA chunk, return whether this chunk is cleaned
    pub async fn clean_multi_vmas(&mut self) -> bool {
        let current_pid = current!().process().pid();
        self.chunk_manager.clean_vmas_with_pid(current_pid).await;
        if self.chunk_manager.is_empty() {
            self.process_set.remove(&current_pid);
            return true;
        } else {
            return false;
        }
    }
}
