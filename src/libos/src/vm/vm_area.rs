use super::*;

use super::page_tracker::PageTracker;
use super::vm_epc::EPCMemType;
use super::vm_perms::VMPerms;
use super::vm_range::VMRange;
use super::vm_util::{
    AlignedZeroPage, FileBacked, PagePolicy, VMInitializer, VMMapOptions, GB, KB, MB,
};
use intrusive_collections::rbtree::{Link, RBTree};
use intrusive_collections::{intrusive_adapter, KeyAdapter};
use std::ops::{Deref, DerefMut};

// Commit memory size unit when the #PF occurs.
const COMMIT_SIZE_UNIT: usize = 4 * KB;
// Commit the whole VMA when this threshold reaches.
const PF_NUM_THRESHOLD: u64 = 3;

#[derive(Clone, Debug)]
pub struct VMArea {
    range: VMRange,
    perms: VMPerms,
    file_backed: Option<FileBacked>,
    access: VMAccess,
    pages: Option<PageTracker>, // Track the paging status of this VMA
    epc_type: EPCMemType,       // Track the type of the EPC to use specific APIs
    pf_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VMAccess {
    /// Can only be accessed by one single process
    Private(pid_t),
    /// Can be accessed by multi processes, also a reference counter
    /// to record sharings within each process(like thread)
    Shared(HashMap<pid_t, u32>),
}

impl VMArea {
    pub fn new(
        range: VMRange,
        perms: VMPerms,
        file_backed: Option<FileBacked>,
        pid: pid_t,
    ) -> Self {
        let epc_type = EPCMemType::new(&range);
        let pages = {
            match epc_type {
                EPCMemType::Reserved => None,
                EPCMemType::UserRegion => {
                    let pages =
                        PageTracker::new_vma_tracker(&range, &EPCMemType::UserRegion).unwrap();
                    (!pages.is_fully_committed()).then_some(pages)
                }
            }
        };

        let new_vma = Self {
            range,
            perms,
            file_backed,
            access: VMAccess::Private(pid),
            pages,
            epc_type,
            pf_count: 0,
        };
        trace!("new vma = {:?}", new_vma);
        new_vma
    }

    fn new_with_page_tracker(
        range: VMRange,
        perms: VMPerms,
        file_backed: Option<FileBacked>,
        access: VMAccess,
        pages: Option<PageTracker>,
    ) -> VMArea {
        let epc_type = EPCMemType::new(&range);
        Self {
            range,
            perms,
            file_backed,
            access,
            pages,
            epc_type,
            pf_count: 0,
        }
    }

    /// Create a new VMArea object that inherits the write-back file (if any), but has
    /// a new range and permissions.
    pub fn inherits_file_from(
        vma: &VMArea,
        new_range: VMRange,
        new_perms: VMPerms,
        access: VMAccess,
    ) -> Self {
        debug_assert!(vma.is_superset_of(&new_range));

        let new_backed_file = if let Some(file) = &vma.file_backed {
            let mut new_file = file.clone();
            let file_offset = file.offset();

            debug_assert!(vma.start() <= new_range.start());
            let new_start_offset = new_range.start() - vma.start();
            let new_file_offset = file_offset + new_start_offset;

            new_file.set_offset(new_file_offset);
            Some(new_file)
        } else {
            None
        };

        let new_pages = {
            let mut new_pages = vma.pages.clone();

            if let Some(pages) = &mut new_pages {
                pages.split_for_new_range(&new_range);
                if pages.is_fully_committed() {
                    None
                } else {
                    new_pages
                }
            } else {
                None
            }
        };

        let new_vma =
            Self::new_with_page_tracker(new_range, new_perms, new_backed_file, access, new_pages);

        trace!("inherits vma: {:?}, create new vma: {:?}", vma, new_vma);
        new_vma
    }

    pub fn perms(&self) -> VMPerms {
        self.perms
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn access(&self) -> &VMAccess {
        &self.access
    }

    pub fn get_private_pid(&self) -> Option<pid_t> {
        match &self.access {
            VMAccess::Private(pid) => Some(*pid),
            VMAccess::Shared(_) => None,
        }
    }

    pub fn belong_to(&self, target_pid: pid_t) -> bool {
        match &self.access {
            VMAccess::Private(pid) => *pid == target_pid,
            VMAccess::Shared(pid_table) => pid_table.contains_key(&target_pid),
        }
    }

    pub fn exclusive_by(&self, target_pid: pid_t) -> bool {
        match &self.access {
            VMAccess::Private(pid) => *pid == target_pid,
            VMAccess::Shared(pid_table) => {
                pid_table.len() == 1
                    && pid_table.contains_key(&target_pid)
                    && *pid_table.get(&target_pid).unwrap() == 1
            }
        }
    }

    fn pages(&self) -> &PageTracker {
        debug_assert!(!self.is_fully_committed());
        self.pages.as_ref().unwrap()
    }

    fn pages_mut(&mut self) -> &mut PageTracker {
        debug_assert!(!self.is_fully_committed());
        self.pages.as_mut().unwrap()
    }

    // Get pid for private VMA
    pub fn pid(&self) -> pid_t {
        match self.access {
            VMAccess::Private(pid) => pid,
            VMAccess::Shared(_) => unreachable!(),
        }
    }

    pub fn is_reserved_only(&self) -> bool {
        if let Some(pages) = &self.pages {
            return pages.is_reserved_only();
        } else {
            false
        }
    }

    pub fn is_fully_committed(&self) -> bool {
        self.pages.is_none()
    }

    pub fn is_partially_committed(&self) -> bool {
        if let Some(pages) = &self.pages {
            return pages.is_partially_committed();
        } else {
            false
        }
    }

    pub fn init_memory(mut self, options: &VMMapOptions) -> Result<Self> {
        let mut vm_area = self;
        let page_policy = options.page_policy();

        // Commit pages if needed
        if !vm_area.is_fully_committed() && page_policy == &PagePolicy::CommitNow {
            vm_area.pages_mut().commit_whole(VMPerms::DEFAULT)?;
            vm_area.pages = None;
        }

        // Initialize committed memory
        if vm_area.is_partially_committed() {
            let committed = true;
            for range in vm_area.pages().get_ranges(committed) {
                vm_area.init_memory_internal(&range, Some(options.initializer()))?;
            }
        } else if vm_area.is_fully_committed() {
            // Initialize the memory of the new range
            unsafe {
                let buf = vm_area.range().as_slice_mut();
                options.initializer().init_slice(buf)?;
            }

            // Set memory permissions
            if !options.perms().is_default() {
                vm_area.modify_permission_force(None, VMPerms::DEFAULT, vm_area.perms());
            }
        }
        // Do nothing if this vma has no committed memory

        Ok(vm_area)
    }

    pub fn flush_and_clean_memory(&self) -> Result<()> {
        let (need_flush, file, file_offset) = match self.writeback_file() {
            None => (false, None, None),
            Some((file_handle, offset)) => {
                if !file_handle.access_mode().unwrap().writable() {
                    (false, None, None)
                } else {
                    (true, Some(file_handle), Some(offset))
                }
            }
        };

        if self.is_fully_committed() {
            self.flush_and_clean_internal(self.range(), need_flush, file, file_offset);
        } else {
            let committed = true;
            for range in self.pages().get_ranges(committed) {
                self.flush_and_clean_internal(&range, need_flush, file, file_offset);
            }
        }

        Ok(())
    }

    fn flush_and_clean_internal(
        &self,
        target_range: &VMRange,
        need_flush: bool,
        file: Option<&FileRef>,
        file_offset: Option<usize>,
    ) {
        trace!("flush and clean committed range: {:?}", target_range);
        debug_assert!(self.range().is_superset_of(target_range));
        let buf = unsafe { target_range.as_slice_mut() };
        if !self.perms().is_default() {
            self.modify_permission_force(Some(&target_range), self.perms(), VMPerms::default());
        }

        if need_flush {
            let file_offset = file_offset.unwrap() + (target_range.start() - self.range.start());
            let file_len = file.unwrap().metadata().unwrap().size;
            if file_offset < file_len {
                let effective_mem_len = std::cmp::min(target_range.size(), file_len - file_offset);
                let len = file
                    .unwrap()
                    .write_at(file_offset, &buf[..effective_mem_len])
                    .expect("flush file failure");
                debug_assert!(len == effective_mem_len);
            } // else file_offset >= file_len, no need to write file
        }

        // reset zeros
        unsafe {
            buf.iter_mut().for_each(|b| *b = 0);
        }
    }

    pub fn modify_permissions_for_committed_pages(&self, curr_perms: VMPerms, new_perms: VMPerms) {
        if self.is_fully_committed() {
            self.modify_permission_force(None, curr_perms, new_perms);
        } else if self.is_partially_committed() {
            let committed = true;
            for range in self.pages().get_ranges(committed) {
                self.modify_permission_force(Some(&range), curr_perms, new_perms);
            }
        }
    }

    pub fn handle_page_fault(
        &mut self,
        rip: usize,
        pf_addr: usize,
        errcd: u32,
        kernel_triggers: bool,
    ) -> Result<()> {
        debug!("PF vma = {:?}", self);
        if (self.perms() == VMPerms::NONE)
            || (crate::exception::check_rw_bit(errcd) == false
                && !self.perms().contains(VMPerms::READ))
        {
            return_errno!(
                EACCES,
                "Page is set to None permission. This is user-intended"
            );
        }

        if crate::exception::check_rw_bit(errcd) && !self.perms().contains(VMPerms::WRITE) {
            return_errno!(
                EACCES, "Page is set to not contain WRITE permission but this PF is triggered by write. This is user-intended"
            )
        }

        if rip == pf_addr && !self.perms().contains(VMPerms::EXEC) {
            return_errno!(
                EACCES, "Page is set to not contain EXEC permission but this PF is triggered by execution. This is user-intended"
            )
        }

        if self.is_fully_committed() {
            // This vma has been commited by other threads already. Just return.
            info!("This vma has been committed by other threads already.");
            return Ok(());
        }

        if matches!(self.epc_type, EPCMemType::Reserved) {
            return_errno!(EINVAL, "reserved memory shouldn't trigger PF");
        }

        if kernel_triggers || self.pf_count >= PF_NUM_THRESHOLD {
            info!("commit whole vma");
            return self.commit_current_vma_whole();
        }

        self.pf_count += 1;
        // The return commit_size can be 0 when other threads already commit the PF-containing range but the vma is not fully committed yet.
        let commit_size = self.commit_once_for_page_fault(pf_addr).unwrap();

        debug!("page fault commit memory size = {:?}", commit_size);

        if commit_size == 0 {
            warn!("This PF has been handled by other threads already.");
        }

        Ok(())
    }

    pub fn backed_file(&self) -> Option<(&FileRef, usize)> {
        if let Some(file) = &self.file_backed {
            Some(file.backed_file())
        } else {
            None
        }
    }

    pub fn writeback_file(&self) -> Option<(&FileRef, usize)> {
        if let Some(file) = &self.file_backed {
            file.writeback_file()
        } else {
            None
        }
    }

    pub fn set_perms(&mut self, new_perms: VMPerms) {
        self.perms = new_perms;
    }

    pub fn subtract(&self, other: &VMRange) -> Vec<VMArea> {
        self.deref()
            .subtract(other)
            .into_iter()
            .map(|range| Self::inherits_file_from(self, range, self.perms(), self.access().clone()))
            .collect()
    }

    // Returns an non-empty intersection if where is any
    pub fn intersect(&self, other: &VMRange) -> Option<VMArea> {
        let new_range = {
            let new_range = self.range().intersect(other);
            if new_range.is_none() {
                return None;
            }
            new_range.unwrap()
        };
        let new_vma =
            VMArea::inherits_file_from(self, new_range, self.perms(), self.access().clone());
        Some(new_vma)
    }

    pub fn set_start(&mut self, new_start: usize) {
        let old_start = self.start();
        if new_start == old_start {
            return;
        }

        self.range.set_start(new_start);

        if new_start < old_start {
            // Extend this VMA
            let pages = {
                let pages = PageTracker::new_vma_tracker(&self.range, &self.epc_type).unwrap();
                (!pages.is_fully_committed()).then_some(pages)
            };
            self.pages = pages;
        } else {
            // Split this VMA
            debug_assert!(new_start > old_start);
            if let Some(pages) = &mut self.pages {
                pages.split_for_new_range(&self.range);
                if pages.is_fully_committed() {
                    self.pages = None;
                }
            }
        }

        if let Some(file) = self.file_backed.as_mut() {
            // If the updates to the VMA needs to write back to a file, then the
            // file offset must be adjusted according to the new start address.
            Self::set_file_offset(file, new_start, old_start);
        }
    }

    fn set_file_offset(file: &mut FileBacked, new_start_offset: usize, old_start_offset: usize) {
        let offset = file.offset();
        if old_start_offset < new_start_offset {
            file.set_offset(offset + (new_start_offset - old_start_offset));
        } else {
            // The caller must guarantee that the new start makes sense
            debug_assert!(offset >= old_start_offset - new_start_offset);
            file.set_offset(offset - (old_start_offset - new_start_offset));
        }
    }

    pub fn is_the_same_to(&self, other: &VMArea) -> bool {
        if self.range() != other.range() {
            return false;
        }

        if self.perms() != other.perms() {
            return false;
        }

        if self.access() != other.access() {
            return false;
        }

        let self_writeback_file = self.writeback_file();
        let other_writeback_file = other.writeback_file();
        match (self_writeback_file, other_writeback_file) {
            (None, None) => return true,
            (Some(_), None) => return false,
            (None, Some(_)) => return false,
            (Some((self_file, self_offset)), Some((other_file, other_offset))) => {
                Arc::ptr_eq(&self_file, &other_file) && self_offset == other_offset
            }
        }
    }

    pub fn set_end(&mut self, new_end: usize) {
        self.range.set_end(new_end);
        let pages = if self.range.size() > 0 {
            let pages = PageTracker::new_vma_tracker(&self.range, &self.epc_type).unwrap();
            (!pages.is_fully_committed()).then_some(pages)
        } else {
            None
        };
        self.pages = pages;
    }

    pub fn can_merge_vmas(left: &VMArea, right: &VMArea) -> bool {
        debug_assert!(left.end() <= right.start());

        // Both of the two VMAs must not be sentry (whose size == 0)
        if left.size() == 0 || right.size() == 0 {
            return false;
        }
        // The two VMAs must border with each other
        if left.end() != right.start() {
            return false;
        }
        // The two VMAs must have the same memory permissions
        if left.perms() != right.perms() {
            return false;
        }
        // The two VMAs must be owned by the same process privately
        // Return false if (either is none) or (both are some but two private pids are different)
        let private_access = left.get_private_pid().zip(right.get_private_pid());
        if private_access.is_none() {
            return false;
        }
        if private_access.is_some_and(|(left_pid, right_pid)| left_pid != right_pid) {
            return false;
        }

        // If the two VMAs have write-back files, the files must be the same and
        // the two file regions must be continuous.
        let left_writeback_file = left.writeback_file();
        let right_writeback_file = right.writeback_file();
        match (left_writeback_file, right_writeback_file) {
            (None, None) => true,
            (Some(_), None) => false,
            (None, Some(_)) => false,
            (Some((left_file, left_offset)), Some((right_file, right_offset))) => {
                Arc::ptr_eq(&left_file, &right_file)
                    && right_offset > left_offset
                    && right_offset - left_offset == left.size()
            }
        }
    }

    /// Flush a file-backed VMA to its file. This has no effect on anonymous VMA.
    pub fn flush_committed_backed_file(&self) {
        self.flush_committed_backed_file_with_cond(|_| true)
    }

    /// Same as `flush_committed_backed_file()`, except that an extra condition on the file needs to satisfy.
    pub fn flush_committed_backed_file_with_cond<F: Fn(&FileRef) -> bool>(&self, cond_fn: F) {
        let (file, file_offset) = match self.writeback_file() {
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
        self.flush_file(file, file_offset)
            .expect("flush memory to file failure");
    }

    fn flush_file(&self, file: &Arc<dyn File>, file_offset: usize) -> Result<()> {
        let file_len = file.metadata().unwrap().size;
        if file_offset >= file_len {
            return Ok(());
        }

        if self.is_fully_committed() {
            let effective_mem_len = std::cmp::min(self.range().size(), file_len - file_offset);
            let len = file.write_at(file_offset, unsafe {
                &self.as_slice()[..effective_mem_len]
            })?;
            debug_assert!(len == effective_mem_len);
        } else {
            let committed = true;
            let vm_range_start = self.range().start();
            for range in self.pages().get_ranges(committed) {
                let file_offset = file_offset + (range.start() - vm_range_start);
                if file_offset >= file_len {
                    break;
                }

                let effective_mem_len = std::cmp::min(range.size(), file_len - file_offset);
                let len = file.write_at(file_offset, unsafe {
                    &range.as_slice()[..effective_mem_len]
                })?;
                debug_assert!(len == effective_mem_len);
            }
        }

        Ok(())
    }

    pub fn is_shared(&self) -> bool {
        match self.access {
            VMAccess::Private(_) => false,
            VMAccess::Shared(_) => true,
        }
    }

    pub fn mark_shared(&mut self) {
        let access = match self.access {
            VMAccess::Private(pid) => VMAccess::Shared(HashMap::from([(pid, 1)])),
            VMAccess::Shared(_) => {
                return;
            }
        };
        self.access = access;
    }

    pub fn attach_shared_process(&mut self, pid: pid_t) -> Result<()> {
        match &mut self.access {
            VMAccess::Private(_) => Err(errno!(EINVAL, "not a shared vma")),
            VMAccess::Shared(pid_table) => {
                if let Some(mut ref_ctr) = pid_table.get_mut(&pid) {
                    *ref_ctr += 1;
                } else {
                    let _ = pid_table.insert(pid, 1);
                }
                Ok(())
            }
        }
    }

    pub fn detach_shared_process(&mut self, pid: pid_t, force_detach: bool) -> Result<bool> {
        match &mut self.access {
            VMAccess::Private(_) => Err(errno!(EINVAL, "not a shared vma")),
            VMAccess::Shared(pid_table) => {
                if let Some(mut ref_ctr) = pid_table.get_mut(&pid) {
                    *ref_ctr -= 1;
                    if *ref_ctr == 0 || force_detach {
                        let _ = pid_table.remove(&pid);
                    }
                }
                Ok(pid_table.is_empty())
            }
        }
    }

    pub fn inherits_access_from(&mut self, vma: &VMArea) {
        self.access = vma.access().clone()
    }

    // Current implementation with "unwrap()" can help us find the error quickly by panicing directly. Also, restoring VM state
    // when this function fails will require some work and is not that simple.
    // TODO: Return with Result instead of "unwrap()"" in this function.
    fn modify_permission_force(
        &self,
        protect_range: Option<&VMRange>,
        current_perms: VMPerms,
        new_perms: VMPerms,
    ) {
        let protect_range = protect_range.unwrap_or_else(|| self.range());

        self.epc_type
            .epc_allocator()
            .modify_permission(
                protect_range.start(),
                protect_range.size(),
                current_perms,
                new_perms,
            )
            .unwrap()
    }

    // With initializer, the memory should be committed already.
    // Without initializer, the memory need to be committed and initialized.
    fn init_memory_internal(
        &mut self,
        target_range: &VMRange,
        initializer: Option<&VMInitializer>,
    ) -> Result<()> {
        debug_assert!(self.range().is_superset_of(target_range));
        trace!("init range = {:?}", target_range);
        let perms = self.perms();
        if let Some(initializer) = initializer {
            match initializer {
                VMInitializer::FileBacked { file } => {
                    let (file, offset) = file.backed_file();
                    let vma_range_start = self.range.start();

                    let init_file_offset = offset + (target_range.start() - vma_range_start);

                    self.init_file_backed_mem(target_range, &file, init_file_offset, perms)?;
                }
                VMInitializer::DoNothing() => {
                    if !self.perms().is_default() {
                        self.modify_permission_force(Some(target_range), VMPerms::DEFAULT, perms);
                    }
                }
                VMInitializer::FillZeros() => {
                    unsafe {
                        let buf = target_range.as_slice_mut();
                        buf.iter_mut().for_each(|b| *b = 0);
                    }
                    if !perms.is_default() {
                        self.modify_permission_force(Some(target_range), VMPerms::DEFAULT, perms);
                    }
                }
                _ => todo!(),
            }
        } else {
            // No initializer, #PF triggered.
            self.init_memory_for_page_fault(target_range)?;
        }

        Ok(())
    }

    fn init_file_backed_mem(
        &mut self,
        target_range: &VMRange,
        file: &FileRef,
        file_offset: usize,
        new_perm: VMPerms,
    ) -> Result<()> {
        if !file.access_mode().unwrap().readable() {
            return_errno!(EBADF, "file is not readable");
        }

        let buf = unsafe { target_range.as_slice_mut() };
        let file_size = file.metadata().unwrap().size;

        let len = file
            .read_at(file_offset, buf)
            .map_err(|_| errno!(EACCES, "failed to init memory from file"))?;

        if !new_perm.is_default() {
            self.modify_permission_force(Some(target_range), VMPerms::DEFAULT, new_perm);
        }

        Ok(())
    }

    fn init_memory_for_page_fault(&mut self, target_range: &VMRange) -> Result<()> {
        let perms = self.perms;
        let init_file = self
            .backed_file()
            .map(|(file, offset)| (file.clone(), offset));
        if let Some((file, offset)) = init_file {
            let vma_range_start = self.range.start();

            let init_file_offset = offset + (target_range.start() - vma_range_start);

            let mut data = AlignedZeroPage::new_page_aligned_vec(target_range.size());
            let _ = file
                .read_at(init_file_offset, data.as_mut_slice())
                .map_err(|_| errno!(EACCES, "failed to init memory from file"))?;
            self.pages.as_mut().unwrap().commit_memory_with_data(
                target_range,
                data.as_slice(),
                perms,
            )?;
        } else {
            // PF triggered, no file-backed memory, just modify protection
            self.pages
                .as_mut()
                .unwrap()
                .commit_range(target_range, Some(perms))?;
        }
        Ok(())
    }

    fn get_commit_once_size(&self) -> usize {
        COMMIT_SIZE_UNIT
    }

    fn commit_once_for_page_fault(&mut self, pf_addr: usize) -> Result<usize> {
        debug_assert!(!self.is_fully_committed());
        let mut early_return = false;
        let mut total_commit_size = 0;
        let vma_range_start = self.range.start();
        let permission = self.perms();
        let committed = false;
        let mut uncommitted_ranges = self.pages().get_ranges(committed);
        let commit_once_size = self.get_commit_once_size();

        for range in uncommitted_ranges
            .iter_mut()
            .skip_while(|range| !range.contains(pf_addr))
        {
            // Skip until first reach the range which contains the pf_addr
            if total_commit_size == 0 {
                debug_assert!(range.contains(pf_addr));
                range.set_start(align_down(pf_addr, PAGE_SIZE));
                range.resize(std::cmp::min(range.size(), commit_once_size));
            } else if range.size() + total_commit_size > commit_once_size {
                // This is not first time commit. Try to commit until reaching the commit_once_size
                range.resize(commit_once_size - total_commit_size);
            }

            self.init_memory_internal(&range, None)?;

            total_commit_size += range.size();
            if total_commit_size >= commit_once_size {
                break;
            }
        }

        if self.pages().is_fully_committed() {
            trace!("vma is fully committed");
            self.pages = None;
        }

        Ok(total_commit_size)
    }

    // Only used to handle PF triggered by the kernel
    fn commit_current_vma_whole(&mut self) -> Result<()> {
        debug_assert!(!self.is_fully_committed());

        let mut uncommitted_ranges = self.pages.as_ref().unwrap().get_ranges(false);
        for range in uncommitted_ranges {
            self.init_memory_internal(&range, None).unwrap();
        }
        self.pages = None;

        Ok(())
    }
}

impl Deref for VMArea {
    type Target = VMRange;

    fn deref(&self) -> &Self::Target {
        &self.range
    }
}

impl Default for VMAccess {
    fn default() -> Self {
        Self::Private(0)
    }
}

#[derive(Clone)]
pub struct VMAObj {
    link: Link,
    vma: VMArea,
}

impl fmt::Debug for VMAObj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.vma)
    }
}

// key adapter for RBTree which is sorted by the start of vma ranges
intrusive_adapter!(pub VMAAdapter = Box<VMAObj>: VMAObj { link : Link });
impl<'a> KeyAdapter<'a> for VMAAdapter {
    type Key = usize;
    fn get_key(&self, vma_obj: &'a VMAObj) -> usize {
        vma_obj.vma.range().start()
    }
}

impl VMAObj {
    pub fn new_vma_obj(vma: VMArea) -> Box<Self> {
        Box::new(Self {
            link: Link::new(),
            vma,
        })
    }

    pub fn vma(&self) -> &VMArea {
        &self.vma
    }
}
