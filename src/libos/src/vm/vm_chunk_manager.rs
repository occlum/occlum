use super::*;

use super::free_space_manager::VMFreeSpaceManager as FreeRangeManager;
use super::vm_area::*;
use super::vm_perms::VMPerms;
use super::vm_util::*;
use std::collections::BTreeSet;

use intrusive_collections::rbtree::{Link, RBTree};
use intrusive_collections::Bound;
use intrusive_collections::RBTreeLink;
use intrusive_collections::{intrusive_adapter, KeyAdapter};

/// Memory chunk manager.
///
/// Chunk is the memory unit for Occlum. For chunks with `default` size, every chunk is managed by a ChunkManager which provides
/// usedful memory management APIs such as mmap, munmap, mremap, mprotect, etc.
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

    pub fn clean_vmas_with_pid(&mut self, pid: pid_t) {
        let mut vmas_cursor = self.vmas.cursor_mut();
        vmas_cursor.move_next(); // move to the first element of the tree
        while !vmas_cursor.is_null() {
            let vma = vmas_cursor.get().unwrap().vma();
            if vma.pid() != pid || vma.size() == 0 {
                // Skip vmas which doesn't belong to this process
                vmas_cursor.move_next();
                continue;
            }

            Self::flush_file_vma(vma);

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

    pub fn mmap(&mut self, options: &VMMapOptions) -> Result<usize> {
        let addr = *options.addr();
        let size = *options.size();
        let align = *options.align();

        if let VMMapAddr::Force(addr) = addr {
            self.munmap(addr, size)?;
        }

        // Find and allocate a new range for this mmap request
        let new_range = self
            .free_manager
            .find_free_range_internal(size, align, addr)?;
        let new_addr = new_range.start();
        let writeback_file = options.writeback_file().clone();
        let current_pid = current!().process().pid();
        let new_vma = VMArea::new(new_range, *options.perms(), writeback_file, current_pid);

        // Initialize the memory of the new range
        unsafe {
            let buf = new_vma.as_slice_mut();
            options.initializer().init_slice(buf)?;
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

    pub fn munmap_range(&mut self, range: VMRange) -> Result<()> {
        let bound = range.start();
        let current_pid = current!().process().pid();

        // The cursor to iterate vmas that might intersect with munmap_range.
        // Upper bound returns the vma whose start address is below and nearest to the munmap range. Start from this range.
        let mut vmas_cursor = self.vmas.upper_bound_mut(Bound::Included(&bound));
        while !vmas_cursor.is_null() && vmas_cursor.get().unwrap().vma().start() <= range.end() {
            let vma = &vmas_cursor.get().unwrap().vma();
            warn!("munmap related vma = {:?}", vma);
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
            Self::flush_file_vma(&intersection_vma);
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
                warn!("intersection vma = {:?}", intersection_vma);
                let buf = intersection_vma.as_slice_mut();
                buf.iter_mut().for_each(|b| *b = 0)
            }

            self.free_manager
                .add_range_back_to_free_manager(intersection_vma.range());
            self.free_size += intersection_vma.size();
        }
        Ok(())
    }

    pub fn munmap(&mut self, addr: usize, size: usize) -> Result<()> {
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

        self.munmap_range(munmap_range)
    }

    pub fn mremap(&mut self, options: &VMRemapOptions) -> Result<usize> {
        let old_addr = options.old_addr();
        let old_size = options.old_size();
        let old_range = VMRange::new_with_size(old_addr, old_size)?;
        let new_size = options.new_size();
        let flags = options.flags();
        let size_type = SizeType::new(&old_size, &new_size);

        return_errno!(ENOSYS, "Under development");

        // Old dead code. Could be used for future development.
        #[cfg(dev)]
        {
            // The old range must be contained in one VMA
            let idx = self
                .find_containing_vma_idx(&old_range)
                .ok_or_else(|| errno!(EFAULT, "invalid range"))?;
            let containing_vma = &self.vmas[idx];
            // Get the memory permissions of the old range
            let perms = containing_vma.perms();
            // Get the write back file of the old range if there is one.
            let writeback_file = containing_vma.writeback_file();

            // FIXME: Current implementation for file-backed memory mremap has limitation that if a SUBRANGE of the previous
            // file-backed mmap with MAP_SHARED is then mremap-ed with MREMAP_MAYMOVE, there will be two vmas that have the same backed file.
            // For Linux, writing to either memory vma or the file will update the other two equally. But we won't be able to support this before
            // we really have paging. Thus, if the old_range is not equal to a recorded vma, we will just return with error.
            if writeback_file.is_some() && &old_range != containing_vma.range() {
                return_errno!(EINVAL, "Known limition")
            }

            // Implement mremap as one optional mmap followed by one optional munmap.
            //
            // The exact arguments for the mmap and munmap are determined by the values of MRemapFlags,
            // SizeType and writeback_file. There is a total of 18 combinations among MRemapFlags and
            // SizeType and writeback_file. As some combinations result in the same mmap and munmap operations,
            // the following code only needs to match below patterns of (MRemapFlags, SizeType, writeback_file)
            // and treat each case accordingly.

            // Determine whether need to do mmap. And when possible, determine the returned address
            let (need_mmap, mut ret_addr) = match (flags, size_type, writeback_file) {
                (MRemapFlags::None, SizeType::Growing, None) => {
                    let vm_initializer_for_new_range = VMInitializer::FillZeros();
                    let mmap_opts = VMMapOptionsBuilder::default()
                        .size(new_size - old_size)
                        .addr(VMMapAddr::Need(old_range.end()))
                        .perms(perms)
                        .initializer(vm_initializer_for_new_range)
                        .build()?;
                    let ret_addr = Some(old_addr);
                    (Some(mmap_opts), ret_addr)
                }
                (MRemapFlags::None, SizeType::Growing, Some((backed_file, offset))) => {
                    // Update writeback file offset
                    let new_writeback_file =
                        Some((backed_file.clone(), offset + containing_vma.size()));
                    let vm_initializer_for_new_range = VMInitializer::LoadFromFile {
                        file: backed_file.clone(),
                        offset: offset + containing_vma.size(), // file-backed mremap should start from the end of previous mmap/mremap file
                    };
                    let mmap_opts = VMMapOptionsBuilder::default()
                        .size(new_size - old_size)
                        .addr(VMMapAddr::Need(old_range.end()))
                        .perms(perms)
                        .initializer(vm_initializer_for_new_range)
                        .writeback_file(new_writeback_file)
                        .build()?;
                    let ret_addr = Some(old_addr);
                    (Some(mmap_opts), ret_addr)
                }
                (MRemapFlags::MayMove, SizeType::Growing, None) => {
                    let prefered_new_range =
                        VMRange::new_with_size(old_addr + old_size, new_size - old_size)?;
                    if self.is_free_range(&prefered_new_range) {
                        // Don't need to move the old range
                        let vm_initializer_for_new_range = VMInitializer::FillZeros();
                        let mmap_ops = VMMapOptionsBuilder::default()
                            .size(prefered_new_range.size())
                            .addr(VMMapAddr::Need(prefered_new_range.start()))
                            .perms(perms)
                            .initializer(vm_initializer_for_new_range)
                            .build()?;
                        (Some(mmap_ops), Some(old_addr))
                    } else {
                        // Need to move old range to a new range and init the new range
                        let vm_initializer_for_new_range =
                            VMInitializer::CopyFrom { range: old_range };
                        let mmap_ops = VMMapOptionsBuilder::default()
                            .size(new_size)
                            .addr(VMMapAddr::Any)
                            .perms(perms)
                            .initializer(vm_initializer_for_new_range)
                            .build()?;
                        // Cannot determine the returned address for now, which can only be obtained after calling mmap
                        let ret_addr = None;
                        (Some(mmap_ops), ret_addr)
                    }
                }
                (MRemapFlags::MayMove, SizeType::Growing, Some((backed_file, offset))) => {
                    let prefered_new_range =
                        VMRange::new_with_size(old_addr + old_size, new_size - old_size)?;
                    if self.is_free_range(&prefered_new_range) {
                        // Don't need to move the old range
                        let vm_initializer_for_new_range = VMInitializer::LoadFromFile {
                            file: backed_file.clone(),
                            offset: offset + containing_vma.size(), // file-backed mremap should start from the end of previous mmap/mremap file
                        };
                        // Write back file should start from new offset
                        let new_writeback_file =
                            Some((backed_file.clone(), offset + containing_vma.size()));
                        let mmap_ops = VMMapOptionsBuilder::default()
                            .size(prefered_new_range.size())
                            .addr(VMMapAddr::Need(prefered_new_range.start()))
                            .perms(perms)
                            .initializer(vm_initializer_for_new_range)
                            .writeback_file(new_writeback_file)
                            .build()?;
                        (Some(mmap_ops), Some(old_addr))
                    } else {
                        // Need to move old range to a new range and init the new range
                        let vm_initializer_for_new_range = {
                            let copy_end = containing_vma.end();
                            let copy_range = VMRange::new(old_range.start(), copy_end)?;
                            let reread_file_start_offset = copy_end - containing_vma.start();
                            VMInitializer::CopyOldAndReadNew {
                                old_range: copy_range,
                                file: backed_file.clone(),
                                offset: reread_file_start_offset,
                            }
                        };
                        let new_writeback_file = Some((backed_file.clone(), *offset));
                        let mmap_ops = VMMapOptionsBuilder::default()
                            .size(new_size)
                            .addr(VMMapAddr::Any)
                            .perms(perms)
                            .initializer(vm_initializer_for_new_range)
                            .writeback_file(new_writeback_file)
                            .build()?;
                        // Cannot determine the returned address for now, which can only be obtained after calling mmap
                        let ret_addr = None;
                        (Some(mmap_ops), ret_addr)
                    }
                }
                (MRemapFlags::FixedAddr(new_addr), _, None) => {
                    let vm_initializer_for_new_range =
                        { VMInitializer::CopyFrom { range: old_range } };
                    let mmap_opts = VMMapOptionsBuilder::default()
                        .size(new_size)
                        .addr(VMMapAddr::Force(new_addr))
                        .perms(perms)
                        .initializer(vm_initializer_for_new_range)
                        .build()?;
                    let ret_addr = Some(new_addr);
                    (Some(mmap_opts), ret_addr)
                }
                (MRemapFlags::FixedAddr(new_addr), _, Some((backed_file, offset))) => {
                    let vm_initializer_for_new_range = {
                        let copy_end = containing_vma.end();
                        let copy_range = VMRange::new(old_range.start(), copy_end)?;
                        let reread_file_start_offset = copy_end - containing_vma.start();
                        VMInitializer::CopyOldAndReadNew {
                            old_range: copy_range,
                            file: backed_file.clone(),
                            offset: reread_file_start_offset,
                        }
                    };
                    let new_writeback_file = Some((backed_file.clone(), *offset));
                    let mmap_opts = VMMapOptionsBuilder::default()
                        .size(new_size)
                        .addr(VMMapAddr::Force(new_addr))
                        .perms(perms)
                        .initializer(vm_initializer_for_new_range)
                        .writeback_file(new_writeback_file)
                        .build()?;
                    let ret_addr = Some(new_addr);
                    (Some(mmap_opts), ret_addr)
                }
                _ => (None, Some(old_addr)),
            };

            let need_munmap = match (flags, size_type) {
                (MRemapFlags::None, SizeType::Shrinking)
                | (MRemapFlags::MayMove, SizeType::Shrinking) => {
                    let unmap_addr = old_addr + new_size;
                    let unmap_size = old_size - new_size;
                    Some((unmap_addr, unmap_size))
                }
                (MRemapFlags::MayMove, SizeType::Growing) => {
                    if ret_addr.is_none() {
                        // We must need to do mmap. Thus unmap the old range
                        Some((old_addr, old_size))
                    } else {
                        // We must choose to reuse the old range. Thus, no need to unmap
                        None
                    }
                }
                (MRemapFlags::FixedAddr(new_addr), _) => {
                    let new_range = VMRange::new_with_size(new_addr, new_size)?;
                    if new_range.overlap_with(&old_range) {
                        return_errno!(EINVAL, "new range cannot overlap with the old one");
                    }
                    Some((old_addr, old_size))
                }
                _ => None,
            };

            // Perform mmap and munmap if needed
            if let Some(mmap_options) = need_mmap {
                let mmap_addr = self.mmap(&mmap_options)?;

                if ret_addr.is_none() {
                    ret_addr = Some(mmap_addr);
                }
            }
            if let Some((addr, size)) = need_munmap {
                self.munmap(addr, size).expect("never fail");
            }

            debug_assert!(ret_addr.is_some());
            Ok(ret_addr.unwrap())
        }
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
                warn!("containing_vma = {:?}", containing_vma);
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
    pub fn msync_by_range(&mut self, sync_range: &VMRange) -> Result<()> {
        if !self.range().is_superset_of(sync_range) {
            return_errno!(ENOMEM, "invalid range");
        }

        // ?FIXME: check if sync_range covers unmapped memory
        for vma_obj in &self.vmas {
            let vma = match vma_obj.vma().intersect(sync_range) {
                None => continue,
                Some(vma) => vma,
            };
            Self::flush_file_vma(&vma);
        }
        Ok(())
    }

    /// Sync all shared, file-backed memory mappings of the given file by flushing
    /// the memory content to the file.
    pub fn msync_by_file(&mut self, sync_file: &FileRef) {
        for vma_obj in &self.vmas {
            let is_same_file = |file: &FileRef| -> bool { Arc::ptr_eq(&file, &sync_file) };
            Self::flush_file_vma_with_cond(&vma_obj.vma(), is_same_file);
        }
    }

    /// Flush a file-backed VMA to its file. This has no effect on anonymous VMA.
    pub fn flush_file_vma(vma: &VMArea) {
        Self::flush_file_vma_with_cond(vma, |_| true)
    }

    /// Same as flush_vma, except that an extra condition on the file needs to satisfy.
    pub fn flush_file_vma_with_cond<F: Fn(&FileRef) -> bool>(vma: &VMArea, cond_fn: F) {
        let (file, file_offset) = match vma.writeback_file().as_ref() {
            None => return,
            Some((file_and_offset)) => file_and_offset,
        };
        let file_writable = file
            .access_mode()
            .map(|ac| ac.writable())
            .unwrap_or_default();
        if !file_writable {
            return;
        }
        if !cond_fn(file) {
            return;
        }
        file.write_at(*file_offset, unsafe { vma.as_slice() });
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
        let totol_size = self.range.size();
        let mut used_size = 0;
        self.vmas
            .iter()
            .for_each(|vma_obj| used_size += vma_obj.vma().size());

        return used_size as f32 / totol_size as f32;
    }

    // Returns whether the requested range is free
    fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.range.is_superset_of(request_range)
            && self
                .vmas
                .iter()
                .any(|vma_obj| vma_obj.vma().range().is_superset_of(request_range) == true)
    }
}

impl Drop for ChunkManager {
    fn drop(&mut self) {
        assert!(self.is_empty());
        assert!(self.free_size == self.range.size());
        assert!(self.free_manager.free_size() == self.range.size());
    }
}
