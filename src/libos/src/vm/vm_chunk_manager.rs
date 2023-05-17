use super::*;

use super::free_space_manager::VMFreeSpaceManager as FreeRangeManager;
use super::vm_area::*;
use super::vm_perms::VMPerms;
use super::vm_util::*;

use intrusive_collections::rbtree::{Link, RBTree};
use intrusive_collections::Bound;
use intrusive_collections::RBTreeLink;
use intrusive_collections::{intrusive_adapter, KeyAdapter};

/// Memory chunk manager.
///
/// Chunk is the memory unit for Occlum. For chunks with `default` size, every chunk is managed by a ChunkManager which provides
/// useful memory management APIs such as mmap, munmap, mremap, mprotect, etc.
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

impl ChunkManager {
    pub fn from(addr: usize, size: usize) -> Result<Self> {
        let range = VMRange::new(addr, addr + size)?;
        let vmas = {
            let start = range.start();
            let end = range.end();
            let start_sentry = {
                let range = VMRange::new_empty(start)?;
                let perms = VMPerms::empty();
                // sentry vma shouldn't belong to any process
                VMAObj::new_vma_obj(VMArea::new(range, perms, None, 0))
            };
            let end_sentry = {
                let range = VMRange::new_empty(end)?;
                let perms = VMPerms::empty();
                VMAObj::new_vma_obj(VMArea::new(range, perms, None, 0))
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

    pub fn is_empty(&self) -> bool {
        self.vmas.iter().count() == 2 // only sentry vmas
    }

    pub async fn clean_vmas_with_pid(&mut self, pid: pid_t) {
        let mut vmas_cursor = self.vmas.cursor_mut();
        vmas_cursor.move_next(); // move to the first element of the tree
        while !vmas_cursor.is_null() {
            let vma = vmas_cursor.get().unwrap().vma();
            if vma.pid() != pid || vma.size() == 0 {
                // Skip vmas which doesn't belong to this process
                vmas_cursor.move_next();
                continue;
            }

            vma.flush_backed_file().await;

            if !vma.perms().is_default() {
                VMPerms::apply_perms(vma, VMPerms::default());
            }

            unsafe {
                let buf = vma.as_slice_mut();
                buf.iter_mut().for_each(|b| *b = 0)
            }

            self.free_manager.add_range_back_to_free_manager(vma);
            self.free_size += vma.size();

            // Remove this vma from vmas list
            vmas_cursor.remove();
        }
    }

    pub async fn mmap(&mut self, options: &VMMapOptions) -> Result<usize> {
        let addr = *options.addr();
        let size = *options.size();
        let align = *options.align();

        if let VMMapAddr::Force(addr) = addr {
            self.munmap(addr, size).await?;
        }

        // Find and allocate a new range for this mmap request
        let new_range = self.free_manager.find_free_range(size, align, addr)?;
        let new_addr = new_range.start();
        let current_pid = current!().process().pid();
        let new_vma = VMArea::new(
            new_range,
            *options.perms(),
            options.initializer().backed_file(),
            current_pid,
        );

        // Initialize the memory of the new range
        let buf = unsafe { new_vma.as_slice_mut() };
        let ret = options.initializer().init_slice(buf).await;
        if let Err(e) = ret {
            // Return the free range before return with error
            self.free_manager
                .add_range_back_to_free_manager(new_vma.range());
            return_errno!(e.errno(), "failed to mmap");
        }

        // Set memory permissions
        if !options.perms().is_default() {
            VMPerms::apply_perms(&new_vma, new_vma.perms());
        }
        self.free_size -= new_vma.size();
        // After initializing, we can safely insert the new VMA
        self.vmas.insert(VMAObj::new_vma_obj(new_vma));
        Ok(new_addr)
    }

    pub async fn munmap_range(&mut self, range: VMRange) -> Result<()> {
        // The bound should be no smaller than the chunk range's start address.
        let bound = range.start().max(self.range.start());

        let current_pid = current!().process().pid();

        // The cursor to iterate vmas that might intersect with munmap_range.
        // Upper bound returns the vma whose start address is below and nearest to the munmap range. Start from this range.
        let mut vmas_cursor = self.vmas.upper_bound_mut(Bound::Included(&bound));
        while !vmas_cursor.is_null() && vmas_cursor.get().unwrap().vma().start() <= range.end() {
            let vma = &vmas_cursor.get().unwrap().vma();
            if vma.size() == 0 || current_pid != vma.pid() {
                vmas_cursor.move_next();
                continue;
            }
            let intersection_vma = match vma.intersect(&range) {
                None => {
                    vmas_cursor.move_next();
                    continue;
                }
                Some(intersection_vma) => intersection_vma,
            };

            // File-backed VMA needs to be flushed upon munmap
            intersection_vma.flush_backed_file().await;
            if !&intersection_vma.perms().is_default() {
                VMPerms::apply_perms(&intersection_vma, VMPerms::default());
            }

            if vma.range() == intersection_vma.range() {
                // Exact match. Just remove.
                vmas_cursor.remove();
            } else {
                // The intersection_vma is a subset of current vma
                let mut remain_vmas = vma.subtract(&intersection_vma);
                if remain_vmas.len() == 1 {
                    let new_obj = VMAObj::new_vma_obj(remain_vmas.pop().unwrap());
                    vmas_cursor.replace_with(new_obj);
                    vmas_cursor.move_next();
                } else {
                    debug_assert!(remain_vmas.len() == 2);
                    let vma_left_part = VMAObj::new_vma_obj(remain_vmas.swap_remove(0));
                    vmas_cursor.replace_with(vma_left_part);
                    let vma_right_part = VMAObj::new_vma_obj(remain_vmas.pop().unwrap());
                    // The new element will be inserted at the correct position in the tree based on its key automatically.
                    vmas_cursor.insert(vma_right_part);
                }
            }

            // Reset zero
            unsafe {
                let buf = intersection_vma.as_slice_mut();
                buf.iter_mut().for_each(|b| *b = 0)
            }

            self.free_manager
                .add_range_back_to_free_manager(intersection_vma.range());
            self.free_size += intersection_vma.size();
        }
        Ok(())
    }

    pub async fn munmap(&mut self, addr: usize, size: usize) -> Result<()> {
        let size = {
            if size == 0 {
                return_errno!(EINVAL, "size of munmap must not be zero");
            }
            align_up(size, PAGE_SIZE)
        };
        let munmap_range = {
            let munmap_range = VMRange::new(addr, addr + size)?;

            let effective_munmap_range_opt = munmap_range.intersect(&self.range);
            if effective_munmap_range_opt.is_none() {
                return Ok(());
            }

            let effective_munmap_range = effective_munmap_range_opt.unwrap();
            if effective_munmap_range.empty() {
                return Ok(());
            }
            effective_munmap_range
        };

        self.munmap_range(munmap_range).await
    }

    pub fn parse_mremap_options(&mut self, options: &VMRemapOptions) -> Result<VMRemapResult> {
        let old_addr = options.old_addr();
        let old_size = options.old_size();
        let old_range = VMRange::new_with_size(old_addr, old_size)?;
        let new_size = options.new_size();
        let flags = options.flags();
        let size_type = VMRemapSizeType::new(&old_size, &new_size);
        let current_pid = current!().process().pid();

        // Merge all connecting VMAs here because the old ranges must corresponds to one VMA
        self.merge_all_vmas();

        let containing_vma = {
            let bound = old_range.start();
            // Get the VMA whose start address is smaller but closest to the old range's start address
            let mut vmas_cursor = self.vmas.upper_bound_mut(Bound::Included(&bound));
            while !vmas_cursor.is_null()
                && vmas_cursor.get().unwrap().vma().start() <= old_range.end()
            {
                let vma = &vmas_cursor.get().unwrap().vma();
                // The old range must be contained in one single VMA
                if vma.pid() == current_pid && vma.is_superset_of(&old_range) {
                    break;
                } else {
                    vmas_cursor.move_next();
                    continue;
                }
            }
            if vmas_cursor.is_null() {
                return_errno!(EFAULT, "old range is not a valid vma range");
            }
            vmas_cursor.get().unwrap().vma().clone()
        };

        return self.parse(options, &containing_vma);
    }

    pub fn mprotect(&mut self, addr: usize, size: usize, new_perms: VMPerms) -> Result<()> {
        let protect_range = VMRange::new_with_size(addr, size)?;
        let bound = protect_range.start();
        let mut containing_vmas = self.vmas.upper_bound_mut(Bound::Included(&bound));
        if containing_vmas.is_null() {
            return_errno!(ENOMEM, "invalid range");
        }
        let current_pid = current!().process().pid();

        // If a mprotect range is not a subrange of one vma, it must be subrange of multiple connecting vmas.
        while !containing_vmas.is_null()
            && containing_vmas.get().unwrap().vma().start() <= protect_range.end()
        {
            let mut containing_vma = containing_vmas.get().unwrap().vma().clone();
            if containing_vma.pid() != current_pid {
                containing_vmas.move_next();
                continue;
            }

            let old_perms = containing_vma.perms();
            if new_perms == old_perms {
                containing_vmas.move_next();
                continue;
            }

            let intersection_vma = match containing_vma.intersect(&protect_range) {
                None => {
                    containing_vmas.move_next();
                    continue;
                }
                Some(intersection_vma) => intersection_vma,
            };

            if intersection_vma.range() == containing_vma.range() {
                // The whole containing_vma is mprotected
                containing_vma.set_perms(new_perms);
                VMPerms::apply_perms(&containing_vma, containing_vma.perms());
                containing_vmas.replace_with(VMAObj::new_vma_obj(containing_vma));
                containing_vmas.move_next();
                continue;
            } else {
                // A subrange of containing_vma is mprotected
                debug_assert!(containing_vma.is_superset_of(&intersection_vma));
                let mut remain_vmas = containing_vma.subtract(&intersection_vma);
                match remain_vmas.len() {
                    2 => {
                        // The containing VMA is divided into three VMAs:
                        // Shrinked old VMA:    [containing_vma.start,     protect_range.start)
                        // New VMA:             [protect_range.start,      protect_range.end)
                        // Another new vma:     [protect_range.end,        containing_vma.end)
                        let old_end = containing_vma.end();
                        let protect_end = protect_range.end();

                        // Shrinked old VMA
                        containing_vma.set_end(protect_range.start());

                        // New VMA
                        let new_vma = VMArea::inherits_file_from(
                            &containing_vma,
                            protect_range,
                            new_perms,
                            current_pid,
                        );
                        VMPerms::apply_perms(&new_vma, new_vma.perms());
                        let new_vma = VMAObj::new_vma_obj(new_vma);

                        // Another new VMA
                        let new_vma2 = {
                            let range = VMRange::new(protect_end, old_end).unwrap();
                            let new_vma = VMArea::inherits_file_from(
                                &containing_vma,
                                range,
                                old_perms,
                                current_pid,
                            );
                            VMAObj::new_vma_obj(new_vma)
                        };

                        containing_vmas.replace_with(VMAObj::new_vma_obj(containing_vma));
                        containing_vmas.insert(new_vma);
                        containing_vmas.insert(new_vma2);
                        // In this case, there is no need to check other vmas.
                        break;
                    }
                    1 => {
                        let remain_vma = remain_vmas.pop().unwrap();
                        if remain_vma.start() == containing_vma.start() {
                            // mprotect right side of the vma
                            containing_vma.set_end(remain_vma.end());
                        } else {
                            // mprotect left side of the vma
                            debug_assert!(remain_vma.end() == containing_vma.end());
                            containing_vma.set_start(remain_vma.start());
                        }
                        let new_vma = VMArea::inherits_file_from(
                            &containing_vma,
                            intersection_vma.range().clone(),
                            new_perms,
                            current_pid,
                        );
                        VMPerms::apply_perms(&new_vma, new_vma.perms());

                        containing_vmas.replace_with(VMAObj::new_vma_obj(containing_vma));
                        containing_vmas.insert(VMAObj::new_vma_obj(new_vma));
                        containing_vmas.move_next();
                        continue;
                    }
                    _ => unreachable!(),
                }
            }
        }

        Ok(())
    }

    /// Sync all shared, file-backed memory mappings in the given range by flushing the
    /// memory content to its underlying file.
    pub async fn msync_by_range(&mut self, sync_range: &VMRange) -> Result<()> {
        if !self.range().is_superset_of(sync_range) {
            return_errno!(ENOMEM, "invalid range");
        }

        // ?FIXME: check if sync_range covers unmapped memory
        for vma_obj in &self.vmas {
            let vma = match vma_obj.vma().intersect(sync_range) {
                None => continue,
                Some(vma) => vma,
            };
            vma.flush_backed_file().await;
        }
        Ok(())
    }

    /// Sync all shared, file-backed memory mappings of the given file by flushing
    /// the memory content to the file.
    pub async fn msync_by_file(&mut self, sync_file: &FileRef) {
        let is_same_file = |file: &FileRef| -> bool { file == sync_file };
        for vma_obj in &self.vmas {
            vma_obj
                .vma()
                .flush_backed_file_with_cond(is_same_file)
                .await;
        }
    }

    pub fn find_mmap_region(&self, addr: usize) -> Result<VMRange> {
        let vma = self.vmas.upper_bound(Bound::Included(&addr));
        if vma.is_null() {
            return_errno!(ESRCH, "no mmap regions that contains the address");
        }
        let vma = vma.get().unwrap().vma();
        if vma.pid() != current!().process().pid() || !vma.contains(addr) {
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

    fn merge_all_vmas(&mut self) {
        let mut vmas_cursor = self.vmas.cursor_mut();
        vmas_cursor.move_next(); // move to the first element of the tree
        while !vmas_cursor.is_null() {
            let vma_a = vmas_cursor.get().unwrap().vma();
            if vma_a.size() == 0 {
                vmas_cursor.move_next();
                continue;
            }

            // Peek next, don't move the cursor
            let vma_b = vmas_cursor.peek_next().get().unwrap().vma().clone();
            if VMArea::can_merge_vmas(vma_a, &vma_b) {
                let merged_vmas = {
                    let mut new_vma = vma_a.clone();
                    new_vma.set_end(vma_b.end());
                    new_vma
                };
                let new_obj = VMAObj::new_vma_obj(merged_vmas);
                vmas_cursor.replace_with(new_obj);
                // Move cursor to vma_b
                vmas_cursor.move_next();
                let removed_vma = *vmas_cursor.remove().unwrap();
                debug_assert!(removed_vma.vma().is_the_same_to(&vma_b));

                // Remove operations makes the cursor go to next element. Move it back
                vmas_cursor.move_prev();
            } else {
                // Can't merge these two vma, just move to next
                vmas_cursor.move_next();
                continue;
            }
        }
    }

    // Returns whether the requested range is free
    fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.free_manager.is_free_range(request_range)
    }
}

impl VMRemapParser for ChunkManager {
    fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.is_free_range(request_range)
    }
}

impl Drop for ChunkManager {
    fn drop(&mut self) {
        assert!(self.is_empty());
        assert!(self.free_size == self.range.size());
        assert!(self.free_manager.free_size() == self.range.size());
    }
}
