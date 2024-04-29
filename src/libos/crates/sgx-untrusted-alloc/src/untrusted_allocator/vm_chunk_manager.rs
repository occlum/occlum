use super::*;

use super::free_space_manager::VMFreeSpaceManager as FreeRangeManager;
use super::vm_area::*;

use intrusive_collections::rbtree::RBTree;
use intrusive_collections::Bound;

use libc::c_void;
cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use libc::ocall::{mmap, munmap};
    } else {
        use libc::{mmap, munmap};
    }
}

/// Memory chunk manager.
///
/// Chunk is the memory unit for the allocator.
/// ChunkManager is implemented basically with two data structures: a red-black tree to track vmas in use and a FreeRangeManager to track
/// ranges which are free.

#[derive(Debug, Default)]
pub struct ChunkManager {
    range: VMRange,
    free_size: usize,
    vmas: RBTree<VMAAdapter>,
    free_manager: FreeRangeManager,
}

#[allow(dead_code)]
impl ChunkManager {
    pub fn new(total_size: usize) -> Result<Self> {
        let start_address = {
            let addr = unsafe {
                mmap(
                    0 as *mut _,
                    total_size,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS,
                    0,
                    0,
                )
            };

            if addr == libc::MAP_FAILED {
                return_errno!(ENOMEM, "allocate new chunk failed");
            }

            let addr = addr as usize;
            assert!(addr.checked_add(total_size).is_some());
            addr
        };

        let range = VMRange::new(start_address, start_address + total_size)?;
        let vmas = RBTree::new(VMAAdapter::new());
        debug!(
            "[untrusted alloc] create a new mem chunk, range = {:?}",
            range
        );
        Ok(ChunkManager {
            range,
            free_size: range.size(),
            vmas,
            free_manager: FreeRangeManager::new(range.clone()),
        })
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn vmas(&self) -> &RBTree<VMAAdapter> {
        &self.vmas
    }

    pub fn free_size(&self) -> &usize {
        &self.free_size
    }

    pub fn is_empty(&self) -> bool {
        self.vmas.iter().count() == 0
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Result<usize> {
        // Find and allocate a new range for this request
        let new_range = self.free_manager.find_free_range_internal(size, align)?;
        let new_addr = new_range.start();
        let new_vma = VMArea::new(new_range);
        self.free_size -= new_vma.size();

        debug!("[untrusted alloc] malloc range = {:?}", new_vma.range());
        self.vmas.insert(VMAObj::new_vma_obj(new_vma));
        Ok(new_addr)
    }

    pub fn free(&mut self, addr: usize) -> Result<()> {
        if addr == 0 {
            return Ok(());
        }

        let mut vmas_cursor = self.vmas.find_mut(&addr);
        if vmas_cursor.is_null() {
            return_errno!(EINVAL, "no vma related was found");
        }

        let vma_obj = vmas_cursor.remove().unwrap();
        let vma = vma_obj.vma();
        debug!("[untrusted alloc] free range = {:?}", vma.range());
        self.free_manager
            .add_range_back_to_free_manager(vma.range())?;
        self.free_size += vma.size();
        Ok(())
    }

    pub fn find_used_mem_region(&self, addr: usize) -> Result<VMRange> {
        let vma = self.vmas.upper_bound(Bound::Included(&addr));
        if vma.is_null() {
            return_errno!(ESRCH, "no mmap regions that contains the address");
        }
        let vma = vma.get().unwrap().vma();
        if !vma.contains(addr) {
            return_errno!(ESRCH, "no mmap regions that contains the address");
        }

        return Ok(vma.range().clone());
    }

    pub fn usage_percentage(&self) -> f32 {
        let total_size = self.range.size();
        let mut used_size = 0;
        self.vmas
            .iter()
            .for_each(|vma_obj| used_size += vma_obj.vma().size());

        return used_size as f32 / total_size as f32;
    }

    // Returns whether the requested range is free
    pub fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.free_manager.is_free_range(request_range)
    }

    pub fn contains(&self, addr: usize) -> bool {
        self.range.contains(addr)
    }
}

impl Drop for ChunkManager {
    fn drop(&mut self) {
        debug_assert!(self.is_empty() == true);
        debug_assert!(self.free_size == self.range.size());
        debug_assert!(self.free_manager.free_size() == self.range.size());
        let ret = unsafe { munmap(self.range().start as *mut c_void, self.range().size()) };
        assert!(ret == 0);
    }
}
