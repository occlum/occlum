use super::*;

use super::free_space_manager::VMFreeSpaceManager as FreeRangeManager;
use super::vm_area::*;

use intrusive_collections::rbtree::RBTree;
use intrusive_collections::Bound;

/// Memory chunk manager.
///
/// Chunk is the memory unit for the allocator. For chunks with `default` size, every chunk is managed by a ChunkManager.
/// ChunkManager is implemented basically with two data structures: a red-black tree to track vmas in use and a FreeRangeManager to track
/// ranges which are free.
/// For vmas-in-use, there are two sentry vmas with zero length at the front and end of the red-black tree.

#[derive(Debug, Default)]
pub struct ChunkManager {
    range: VMRange,
    free_size: usize,
    vmas: RBTree<VMAAdapter>,
    free_manager: FreeRangeManager,
}

#[allow(dead_code)]
impl ChunkManager {
    pub fn from(addr: usize, size: usize) -> Result<Self> {
        let range = VMRange::new(addr, addr + size)?;
        let vmas = {
            let start = range.start();
            let end = range.end();
            let start_sentry = {
                let range = VMRange::new_empty(start)?;
                VMAObj::new_vma_obj(VMArea::new(range))
            };
            let end_sentry = {
                let range = VMRange::new_empty(end)?;
                VMAObj::new_vma_obj(VMArea::new(range))
            };
            let mut new_tree = RBTree::new(VMAAdapter::new());
            new_tree.insert(start_sentry);
            new_tree.insert(end_sentry);
            new_tree
        };
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

    pub fn check_empty(&self) -> bool {
        self.vmas.iter().count() == 2 // only sentry vmas
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
        let totol_size = self.range.size();
        let mut used_size = 0;
        self.vmas
            .iter()
            .for_each(|vma_obj| used_size += vma_obj.vma().size());

        return used_size as f32 / totol_size as f32;
    }

    // Returns whether the requested range is free
    pub fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.free_manager.is_free_range(request_range)
    }
}

impl Drop for ChunkManager {
    fn drop(&mut self) {
        debug_assert!(self.check_empty() == true);
        debug_assert!(self.free_size == self.range.size());
        debug_assert!(self.free_manager.free_size() == self.range.size());
    }
}
