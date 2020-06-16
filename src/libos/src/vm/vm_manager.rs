use super::*;

use super::vm_area::VMArea;
use super::vm_perms::VMPerms;

#[derive(Clone, Debug)]
pub enum VMInitializer {
    DoNothing(),
    FillZeros(),
    CopyFrom { range: VMRange },
    LoadFromFile { file: FileRef, offset: usize },
}

impl Default for VMInitializer {
    fn default() -> VMInitializer {
        VMInitializer::DoNothing()
    }
}

impl VMInitializer {
    pub fn init_slice(&self, buf: &mut [u8]) -> Result<()> {
        match self {
            VMInitializer::DoNothing() => {
                // Do nothing
            }
            VMInitializer::FillZeros() => {
                for b in buf {
                    *b = 0;
                }
            }
            VMInitializer::CopyFrom { range } => {
                let src_slice = unsafe { range.as_slice() };
                let copy_len = min(buf.len(), src_slice.len());
                buf[..copy_len].copy_from_slice(&src_slice[..copy_len]);
                for b in &mut buf[copy_len..] {
                    *b = 0;
                }
            }
            VMInitializer::LoadFromFile { file, offset } => {
                // TODO: make sure that read_at does not move file cursor
                let len = file
                    .read_at(*offset, buf)
                    .cause_err(|_| errno!(EIO, "failed to init memory from file"))?;
                for b in &mut buf[len..] {
                    *b = 0;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMMapAddr {
    Any,          // Free to choose any address
    Hint(usize),  // Prefer the address, but can use other address
    Need(usize),  // Need to use the address, otherwise report error
    Force(usize), // Force using the address by munmap first
}

impl Default for VMMapAddr {
    fn default() -> VMMapAddr {
        VMMapAddr::Any
    }
}

#[derive(Builder, Debug, Default)]
#[builder(build_fn(skip), no_std)]
pub struct VMMapOptions {
    size: usize,
    align: usize,
    perms: VMPerms,
    addr: VMMapAddr,
    initializer: VMInitializer,
}

// VMMapOptionsBuilder is generated automatically, except the build function
impl VMMapOptionsBuilder {
    pub fn build(&self) -> Result<VMMapOptions> {
        let size = {
            let size = self
                .size
                .ok_or_else(|| errno!(EINVAL, "invalid size for mmap"))?;
            if size == 0 {
                return_errno!(EINVAL, "invalid size for mmap");
            }
            align_up(size, PAGE_SIZE)
        };
        let align = {
            let align = self.align.unwrap_or(PAGE_SIZE);
            if align == 0 || align % PAGE_SIZE != 0 {
                return_errno!(EINVAL, "invalid size for mmap");
            }
            align
        };
        let perms = self
            .perms
            .ok_or_else(|| errno!(EINVAL, "perms must be given"))?;
        let addr = {
            let addr = self.addr.unwrap_or_default();
            match addr {
                // TODO: check addr + size overflow
                VMMapAddr::Any => VMMapAddr::Any,
                VMMapAddr::Hint(addr) => {
                    let addr = align_down(addr, PAGE_SIZE);
                    VMMapAddr::Hint(addr)
                }
                VMMapAddr::Need(addr_) | VMMapAddr::Force(addr_) => {
                    if addr_ % align != 0 {
                        return_errno!(EINVAL, "unaligned addr for fixed mmap");
                    }
                    addr
                }
            }
        };
        let initializer = match self.initializer.as_ref() {
            Some(initializer) => initializer.clone(),
            None => VMInitializer::default(),
        };
        Ok(VMMapOptions {
            size,
            align,
            perms,
            addr,
            initializer,
        })
    }
}

impl VMMapOptions {
    pub fn size(&self) -> &usize {
        &self.size
    }

    pub fn addr(&self) -> &VMMapAddr {
        &self.addr
    }

    pub fn perms(&self) -> &VMPerms {
        &self.perms
    }

    pub fn initializer(&self) -> &VMInitializer {
        &self.initializer
    }
}

#[derive(Debug)]
pub struct VMRemapOptions {
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: MRemapFlags,
}

impl VMRemapOptions {
    pub fn new(
        old_addr: usize,
        old_size: usize,
        new_size: usize,
        flags: MRemapFlags,
    ) -> Result<Self> {
        let old_addr = if old_addr % PAGE_SIZE != 0 {
            return_errno!(EINVAL, "unaligned old address");
        } else {
            old_addr
        };
        let old_size = if old_size == 0 {
            // TODO: support old_size is zero for shareable mapping
            warn!("do not support old_size is zero");
            return_errno!(EINVAL, "invalid old size");
        } else {
            align_up(old_size, PAGE_SIZE)
        };
        if let Some(new_addr) = flags.new_addr() {
            if new_addr % PAGE_SIZE != 0 {
                return_errno!(EINVAL, "unaligned new address");
            }
        }
        let new_size = if new_size == 0 {
            return_errno!(EINVAL, "invalid new size");
        } else {
            align_up(new_size, PAGE_SIZE)
        };
        Ok(Self {
            old_addr,
            old_size,
            new_size,
            flags,
        })
    }

    pub fn old_addr(&self) -> usize {
        self.old_addr
    }

    pub fn old_size(&self) -> usize {
        self.old_size
    }

    pub fn new_size(&self) -> usize {
        self.new_size
    }

    pub fn flags(&self) -> MRemapFlags {
        self.flags
    }
}

/// Memory manager.
///
/// VMManager provides useful memory management APIs such as mmap, munmap, mremap, etc.
///
/// # Invariants
///
/// Behind the scene, VMManager maintains a list of VMArea that have been allocated.
/// (denoted as `self.vmas`). To reason about the correctness of VMManager, we give
/// the set of invariants hold by VMManager.
///
/// 1. The rule of sentry:
/// ```
/// self.range.begin() == self.vmas[0].start() == self.vmas[0].end()
/// ```
/// and
/// ```
/// self.range.end() == self.vmas[N-1].start() == self.vmas[N-1].end()
/// ```
/// where `N = self.vmas.len()`.
///
/// 2. The rule of non-emptyness:
/// ```
/// self.vmas[i].size() > 0, for 1 <= i < self.vmas.len() - 1
/// ```
///
/// 3. The rule of ordering:
/// ```
/// self.vmas[i].end() <= self.vmas[i+1].start() for 0 <= i < self.vmas.len() - 1
/// ```
///
/// 4. The rule of non-mergablility:
/// ```
/// self.vmas[i].end() !=  self.vmas[i+1].start() || self.vmas[i].perms() !=  self.vmas[i+1].perms()
///     for 1 <= i < self.vmas.len() - 2
/// ```
///
#[derive(Debug, Default)]
pub struct VMManager {
    range: VMRange,
    vmas: Vec<VMArea>,
}

impl VMManager {
    pub fn from(addr: usize, size: usize) -> Result<VMManager> {
        let range = VMRange::new(addr, addr + size)?;
        let vmas = {
            let start = range.start();
            let end = range.end();
            let start_sentry = {
                let range = VMRange::new_empty(start)?;
                let perms = VMPerms::empty();
                VMArea::new(range, perms)
            };
            let end_sentry = {
                let range = VMRange::new_empty(end)?;
                let perms = VMPerms::empty();
                VMArea::new(range, perms)
            };
            vec![start_sentry, end_sentry]
        };
        Ok(VMManager { range, vmas })
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn mmap(&mut self, options: &VMMapOptions) -> Result<usize> {
        // TODO: respect options.align when mmap
        let addr = *options.addr();
        let size = *options.size();

        if let VMMapAddr::Force(addr) = addr {
            self.munmap(addr, size)?;
        }

        // Allocate a new range for this mmap request
        let (insert_idx, free_range) = self.find_free_range(size, addr)?;
        let new_range = self.alloc_range_from(size, addr, &free_range);
        let new_addr = new_range.start();
        let new_vma = VMArea::new(new_range, *options.perms());

        // Initialize the memory of the new range
        unsafe {
            let buf = new_vma.as_slice_mut();
            options.initializer.init_slice(buf)?;
        }
        // Set memory permissions
        Self::apply_perms(&new_vma, new_vma.perms());

        // After initializing, we can safely insert the new VMA
        self.insert_new_vma(insert_idx, new_vma);

        Ok(new_addr)
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

        let new_vmas = self
            .vmas
            .iter()
            .flat_map(|vma| {
                // Keep the two sentry VMA intact
                if vma.size() == 0 {
                    return vec![*vma];
                }

                let intersection_range = match vma.intersect(&munmap_range) {
                    None => return vec![*vma],
                    Some(intersection_range) => intersection_range,
                };

                // Reset memory permissions
                Self::apply_perms(&intersection_range, VMPerms::default());

                vma.subtract(&intersection_range)
            })
            .collect();
        self.vmas = new_vmas;
        Ok(())
    }

    pub fn mremap(&mut self, options: &VMRemapOptions) -> Result<usize> {
        let old_addr = options.old_addr();
        let old_size = options.old_size();
        let old_range = VMRange::new_with_size(old_addr, old_size)?;
        let new_size = options.new_size();
        let flags = options.flags();

        #[derive(Clone, Copy, PartialEq)]
        enum SizeType {
            Same,
            Shrinking,
            Growing,
        };
        let size_type = if new_size == old_size {
            SizeType::Same
        } else if new_size < old_size {
            SizeType::Shrinking
        } else {
            SizeType::Growing
        };

        // Get the memory permissions of the old range
        let perms = {
            // The old range must be contained in one VMA
            let idx = self
                .find_containing_vma_idx(&old_range)
                .ok_or_else(|| errno!(EFAULT, "invalid range"))?;
            let containing_vma = &self.vmas[idx];
            containing_vma.perms()
        };

        // Implement mremap as one optional mmap followed by one optional munmap.
        //
        // The exact arguments for the mmap and munmap are determined by the values of MRemapFlags
        // and SizeType. There is a total of 9 combinations between MRemapFlags and SizeType.
        // As some combinations result in the same mmap and munmap operations, the following code
        // only needs to match four patterns of (MRemapFlags, SizeType) and treat each case
        // accordingly.

        // Determine whether need to do mmap. And when possible, determine the returned address
        // TODO: should fill zeros even when extending a file-backed mapping?
        let (need_mmap, mut ret_addr) = match (flags, size_type) {
            (MRemapFlags::None, SizeType::Growing) => {
                let mmap_opts = VMMapOptionsBuilder::default()
                    .size(new_size - old_size)
                    .addr(VMMapAddr::Need(old_range.end()))
                    .perms(perms)
                    .initializer(VMInitializer::FillZeros())
                    .build()?;
                let ret_addr = Some(old_addr);
                (Some(mmap_opts), ret_addr)
            }
            (MRemapFlags::MayMove, SizeType::Growing) => {
                let prefered_new_range =
                    VMRange::new_with_size(old_addr + old_size, new_size - old_size)?;
                if self.is_free_range(&prefered_new_range) {
                    let mmap_ops = VMMapOptionsBuilder::default()
                        .size(prefered_new_range.size())
                        .addr(VMMapAddr::Need(prefered_new_range.start()))
                        .perms(perms)
                        .initializer(VMInitializer::FillZeros())
                        .build()?;
                    (Some(mmap_ops), Some(old_addr))
                } else {
                    let mmap_ops = VMMapOptionsBuilder::default()
                        .size(new_size)
                        .addr(VMMapAddr::Any)
                        .perms(perms)
                        .initializer(VMInitializer::CopyFrom { range: old_range })
                        .build()?;
                    // Cannot determine the returned address for now, which can only be obtained after calling mmap
                    let ret_addr = None;
                    (Some(mmap_ops), ret_addr)
                }
            }
            (MRemapFlags::FixedAddr(new_addr), _) => {
                let mmap_opts = VMMapOptionsBuilder::default()
                    .size(new_size)
                    .addr(VMMapAddr::Force(new_addr))
                    .perms(perms)
                    .initializer(VMInitializer::CopyFrom { range: old_range })
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

    pub fn mprotect(&mut self, addr: usize, size: usize, new_perms: VMPerms) -> Result<()> {
        let protect_range = VMRange::new_with_size(addr, size)?;

        // FIXME: the current implementation requires the target range to be
        // contained in exact one VMA.
        let containing_idx = self
            .find_containing_vma_idx(&protect_range)
            .ok_or_else(|| errno!(ENOMEM, "invalid range"))?;
        let containing_vma = &self.vmas[containing_idx];

        let old_perms = containing_vma.perms();
        if new_perms == old_perms {
            return Ok(());
        }

        let same_start = protect_range.start() == containing_vma.start();
        let same_end = protect_range.end() == containing_vma.end();
        let containing_vma = &mut self.vmas[containing_idx];
        match (same_start, same_end) {
            (true, true) => {
                containing_vma.set_perms(new_perms);

                Self::apply_perms(containing_vma, containing_vma.perms());
            }
            (false, true) => {
                containing_vma.set_end(protect_range.start());

                let new_vma = VMArea::new(protect_range, new_perms);
                Self::apply_perms(&new_vma, new_vma.perms());
                self.insert_new_vma(containing_idx + 1, new_vma);
            }
            (true, false) => {
                containing_vma.set_start(protect_range.end());

                let new_vma = VMArea::new(protect_range, new_perms);
                Self::apply_perms(&new_vma, new_vma.perms());
                self.insert_new_vma(containing_idx, new_vma);
            }
            (false, false) => {
                // The containing VMA is divided into three VMAs:
                // Shrinked old VMA:    [containing_vma.start,     protect_range.start)
                // New VMA:             [protect_range.start,      protect_range.end)
                // Another new vma:     [protect_range.end,        containing_vma.end)

                let old_end = containing_vma.end();
                let protect_end = protect_range.end();

                // Shrinked old VMA
                containing_vma.set_end(protect_range.start());

                // New VMA
                let new_vma = VMArea::new(protect_range, new_perms);
                Self::apply_perms(&new_vma, new_vma.perms());
                self.insert_new_vma(containing_idx + 1, new_vma);

                // Another new VMA
                let new_vma2 = {
                    let range = VMRange::new(protect_end, old_end).unwrap();
                    VMArea::new(range, old_perms)
                };
                self.insert_new_vma(containing_idx + 2, new_vma2);
            }
        }

        Ok(())
    }

    pub fn find_mmap_region(&self, addr: usize) -> Result<&VMRange> {
        self.vmas
            .iter()
            .map(|vma| vma.range())
            .find(|vma| vma.contains(addr))
            .ok_or_else(|| errno!(ESRCH, "no mmap regions that contains the address"))
    }

    // Find a VMA that contains the given range, returning the VMA's index
    fn find_containing_vma_idx(&self, target_range: &VMRange) -> Option<usize> {
        self.vmas
            .iter()
            .position(|vma| vma.is_superset_of(target_range))
    }

    // Returns whether the requested range is free
    fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.range.is_superset_of(request_range)
            && self
                .vmas
                .iter()
                .all(|range| range.overlap_with(request_range) == false)
    }

    // Find the free range that satisfies the constraints of size and address
    fn find_free_range(&self, size: usize, addr: VMMapAddr) -> Result<(usize, VMRange)> {
        // TODO: reduce the complexity from O(N) to O(log(N)), where N is
        // the number of existing VMAs.

        // Record the minimal free range that satisfies the contraints
        let mut result_free_range: Option<VMRange> = None;
        let mut result_idx: Option<usize> = None;

        for (idx, range_pair) in self.vmas.windows(2).enumerate() {
            // Since we have two sentry vmas at both ends, we can be sure that the free
            // space only appears between two consecutive vmas.
            let pre_range = &range_pair[0];
            let next_range = &range_pair[1];

            let mut free_range = {
                let free_range_start = pre_range.end();
                let free_range_end = next_range.start();

                let free_range_size = free_range_end - free_range_start;
                if free_range_size < size {
                    continue;
                }

                unsafe { VMRange::from_unchecked(free_range_start, free_range_end) }
            };

            match addr {
                // Want a minimal free_range
                VMMapAddr::Any => {}
                // Prefer to have free_range.start == addr
                VMMapAddr::Hint(addr) => {
                    if free_range.contains(addr) {
                        if free_range.end() - addr >= size {
                            free_range.start = addr;
                            let insert_idx = idx + 1;
                            return Ok((insert_idx, free_range));
                        }
                    }
                }
                // Must have free_range.start == addr
                VMMapAddr::Need(addr) | VMMapAddr::Force(addr) => {
                    if free_range.start() > addr {
                        return_errno!(ENOMEM, "not enough memory for fixed mmap");
                    }
                    if !free_range.contains(addr) {
                        continue;
                    }
                    if free_range.end() - addr < size {
                        return_errno!(ENOMEM, "not enough memory for fixed mmap");
                    }
                    free_range.start = addr;
                    let insert_idx = idx + 1;
                    return Ok((insert_idx, free_range));
                }
            }

            if result_free_range == None
                || result_free_range.as_ref().unwrap().size() > free_range.size()
            {
                result_free_range = Some(free_range);
                result_idx = Some(idx);
            }
        }

        if result_free_range.is_none() {
            return_errno!(ENOMEM, "not enough memory");
        }

        let free_range = result_free_range.unwrap();
        let insert_idx = result_idx.unwrap() + 1;
        Ok((insert_idx, free_range))
    }

    fn alloc_range_from(&self, size: usize, addr: VMMapAddr, free_range: &VMRange) -> VMRange {
        debug_assert!(free_range.size() >= size);

        let mut new_range = *free_range;

        if let VMMapAddr::Need(addr) = addr {
            debug_assert!(addr == new_range.start());
        }
        if let VMMapAddr::Force(addr) = addr {
            debug_assert!(addr == new_range.start());
        }

        new_range.resize(size);
        new_range
    }

    // Insert a new VMA, and when possible, merge it with its neighbors.
    fn insert_new_vma(&mut self, insert_idx: usize, new_vma: VMArea) {
        // New VMA can only be inserted between the two sentry VMAs
        debug_assert!(0 < insert_idx && insert_idx < self.vmas.len());

        let left_idx = insert_idx - 1;
        let right_idx = insert_idx;

        let left_vma = &self.vmas[left_idx];
        let right_vma = &self.vmas[right_idx];

        // Double check the order
        debug_assert!(left_vma.end() <= new_vma.start());
        debug_assert!(new_vma.end() <= right_vma.start());

        let left_mergable = Self::can_merge_vmas(left_vma, &new_vma);
        let right_mergable = Self::can_merge_vmas(&new_vma, right_vma);

        drop(left_vma);
        drop(right_vma);

        match (left_mergable, right_mergable) {
            (false, false) => {
                self.vmas.insert(insert_idx, new_vma);
            }
            (true, false) => {
                self.vmas[left_idx].set_end(new_vma.end);
            }
            (false, true) => {
                self.vmas[right_idx].set_start(new_vma.start);
            }
            (true, true) => {
                let left_new_end = self.vmas[right_idx].end();
                self.vmas[left_idx].set_end(left_new_end);
                self.vmas.remove(right_idx);
            }
        }
    }

    fn can_merge_vmas(left: &VMArea, right: &VMArea) -> bool {
        debug_assert!(left.end() <= right.start());

        // Both of the two VMAs are not sentry (whose size == 0)
        left.size() > 0 && right.size() > 0 &&
            // Two VMAs must border with each other
            left.end() == right.start() &&
            // Two VMAs must have the same memory permissions
            left.perms() == right.perms()
    }

    fn apply_perms(protect_range: &VMRange, perms: VMPerms) {
        extern "C" {
            pub fn occlum_ocall_mprotect(
                retval: *mut i32,
                addr: *const c_void,
                len: usize,
                prot: i32,
            ) -> sgx_status_t;
        };

        unsafe {
            let mut retval = 0;
            let addr = protect_range.start() as *const c_void;
            let len = protect_range.size();
            let prot = perms.bits() as i32;
            let sgx_status = occlum_ocall_mprotect(&mut retval, addr, len, prot);
            assert!(sgx_status == sgx_status_t::SGX_SUCCESS && retval == 0);
        }
    }
}

impl Drop for VMManager {
    fn drop(&mut self) {
        // Ensure that memory permissions are recovered
        for vma in &self.vmas {
            if vma.size() == 0 || vma.perms() == VMPerms::default() {
                continue;
            }
            Self::apply_perms(vma, VMPerms::default());
        }
    }
}
