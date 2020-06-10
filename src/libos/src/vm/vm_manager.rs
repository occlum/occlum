use super::*;

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
/// Behind the scene, VMManager maintains a list of VMRange that have been allocated.
/// (denoted as `self.sub_ranges`). To reason about the correctness of VMManager, we give
/// the set of invariants hold by VMManager.
///
/// 1. The rule of sentry:
/// ```
/// self.range.begin() == self.sub_ranges[0].start() == self.sub_ranges[0].end()
/// ```
/// and
/// ```
/// self.range.end() == self.sub_ranges[N-1].start() == self.sub_ranges[N-1].end()
/// ```
/// where `N = self.sub_ranges.len()`.
///
/// 2. The rule of non-emptyness:
/// ```
/// self.sub_ranges[i].size() > 0, for 1 <= i < self.sub_ranges.len() - 1
/// ```
///
/// 3. The rule of ordering:
/// ```
/// self.sub_ranges[i].end() <= self.sub_ranges[i+1].start() for 0 <= i < self.sub_ranges.len() - 1
/// ```
///
/// 4. The rule of non-mergablility:
/// ```
/// self.sub_ranges[i].end() !=  self.sub_ranges[i+1].start() for 1 <= i < self.sub_ranges.len() - 2
/// ```
///
#[derive(Debug, Default)]
pub struct VMManager {
    range: VMRange,
    sub_ranges: Vec<VMRange>,
}

impl VMManager {
    pub fn from(addr: usize, size: usize) -> Result<VMManager> {
        let range = VMRange::new(addr, addr + size)?;
        let sub_ranges = {
            let start = range.start();
            let end = range.end();
            let start_sentry = VMRange::new(start, start)?;
            let end_sentry = VMRange::new(end, end)?;
            vec![start_sentry, end_sentry]
        };
        Ok(VMManager { range, sub_ranges })
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

        // Allocate a new subrange for this mmap request
        let (insert_idx, free_subrange) = self.find_free_subrange(size, addr)?;
        let new_subrange = self.alloc_subrange_from(size, addr, &free_subrange);
        let new_subrange_addr = new_subrange.start();

        // Initialize the memory of the new subrange
        unsafe {
            let buf = new_subrange.as_slice_mut();
            options.initializer.init_slice(buf)?;
        }

        // After initializing, we can safely insert the new subrange
        self.insert_new_subrange(insert_idx, new_subrange);

        Ok(new_subrange_addr)
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

        let new_sub_ranges = self
            .sub_ranges
            .iter()
            .flat_map(|subrange| {
                // Keep the two sentry subranges intact
                if subrange.size() == 0 {
                    return vec![*subrange];
                }

                let unmapped_subrange = match subrange.intersect(&munmap_range) {
                    None => return vec![*subrange],
                    Some(unmapped_subrange) => unmapped_subrange,
                };

                subrange.subtract(&unmapped_subrange)
            })
            .collect();
        self.sub_ranges = new_sub_ranges;
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

        // The old range must not span over multiple sub-ranges
        self.find_containing_subrange_idx(&old_range)
            .ok_or_else(|| errno!(EFAULT, "invalid range"))?;

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
                        .initializer(VMInitializer::FillZeros())
                        .build()?;
                    (Some(mmap_ops), Some(old_addr))
                } else {
                    let mmap_ops = VMMapOptionsBuilder::default()
                        .size(new_size)
                        .addr(VMMapAddr::Any)
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

    pub fn find_mmap_region(&self, addr: usize) -> Result<&VMRange> {
        self.sub_ranges
            .iter()
            .find(|subrange| subrange.contains(addr))
            .ok_or_else(|| errno!(ESRCH, "no mmap regions that contains the address"))
    }

    // Find a subrange that contains the given range and returns the index of the subrange
    fn find_containing_subrange_idx(&self, target_range: &VMRange) -> Option<usize> {
        self.sub_ranges
            .iter()
            .position(|subrange| subrange.is_superset_of(target_range))
    }

    // Returns whether the requested range is free
    fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.range.is_superset_of(request_range)
            && self
                .sub_ranges
                .iter()
                .all(|range| range.overlap_with(request_range) == false)
    }

    // Find the free subrange that satisfies the constraints of size and address
    fn find_free_subrange(&self, size: usize, addr: VMMapAddr) -> Result<(usize, VMRange)> {
        // TODO: reduce the complexity from O(N) to O(log(N)), where N is
        // the number of existing subranges.

        // Record the minimal free range that satisfies the contraints
        let mut result_free_range: Option<VMRange> = None;
        let mut result_idx: Option<usize> = None;

        for (idx, range_pair) in self.sub_ranges.windows(2).enumerate() {
            // Since we have two sentry sub_ranges at both ends, we can be sure that the free
            // space only appears between two consecutive sub_ranges.
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

    fn alloc_subrange_from(
        &self,
        size: usize,
        addr: VMMapAddr,
        free_subrange: &VMRange,
    ) -> VMRange {
        debug_assert!(free_subrange.size() >= size);

        let mut new_subrange = *free_subrange;

        if let VMMapAddr::Need(addr) = addr {
            debug_assert!(addr == new_subrange.start());
        }
        if let VMMapAddr::Force(addr) = addr {
            debug_assert!(addr == new_subrange.start());
        }

        new_subrange.resize(size);
        new_subrange
    }

    // Insert the new sub-range, and when possible, merge it with its neighbors.
    fn insert_new_subrange(&mut self, insert_idx: usize, new_subrange: VMRange) {
        // New sub-range can only be inserted between the two sentry sub-ranges
        debug_assert!(0 < insert_idx && insert_idx < self.sub_ranges.len());

        let left_idx = insert_idx - 1;
        let right_idx = insert_idx;

        // Double check the order
        debug_assert!(self.sub_ranges[left_idx].end() <= new_subrange.start());
        debug_assert!(new_subrange.end() <= self.sub_ranges[right_idx].start());

        let left_mergable = if left_idx > 0 {
            // Mergable if there is no gap between the left neighbor and the new sub-range
            self.sub_ranges[left_idx].end() == new_subrange.start()
        } else {
            // The left sentry sub-range is NOT mergable with any sub-range
            false
        };
        let right_mergable = if right_idx < self.sub_ranges.len() - 1 {
            // Mergable if there is no gap between the right neighbor and the new sub-range
            self.sub_ranges[right_idx].start() == new_subrange.end()
        } else {
            // The right sentry sub-range is NOT mergable with any sub-range
            false
        };

        match (left_mergable, right_mergable) {
            (false, false) => {
                self.sub_ranges.insert(insert_idx, new_subrange);
            }
            (true, false) => {
                self.sub_ranges[left_idx].end = new_subrange.end;
            }
            (false, true) => {
                self.sub_ranges[right_idx].start = new_subrange.start;
            }
            (true, true) => {
                self.sub_ranges[left_idx].end = self.sub_ranges[right_idx].end;
                self.sub_ranges.remove(right_idx);
            }
        }
    }
}
