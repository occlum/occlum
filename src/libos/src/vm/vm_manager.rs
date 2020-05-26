use super::*;

#[derive(Clone, Debug)]
pub enum VMInitializer {
    DoNothing(),
    FillZeros(),
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
    Hint(usize),  // Prefer the given address
    Fixed(usize), // Must be the given address
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
                VMMapAddr::Fixed(addr) => {
                    if addr % align != 0 {
                        return_errno!(EINVAL, "unaligned addr for fixed mmap");
                    }
                    VMMapAddr::Fixed(addr)
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
    new_addr: Option<usize>,
    new_size: usize,
    flags: MRemapFlags,
}

impl VMRemapOptions {
    pub fn new(
        old_addr: usize,
        old_size: usize,
        new_addr: Option<usize>,
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
        let new_addr = {
            if let Some(addr) = new_addr {
                if addr % PAGE_SIZE != 0 {
                    return_errno!(EINVAL, "unaligned new address");
                }
            }
            new_addr
        };
        let new_size = if new_size == 0 {
            return_errno!(EINVAL, "invalid new size");
        } else {
            align_up(new_size, PAGE_SIZE)
        };
        Ok(Self {
            old_addr,
            old_size,
            new_addr,
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

    pub fn new_addr(&self) -> Option<usize> {
        self.new_addr
    }

    pub fn flags(&self) -> MRemapFlags {
        self.flags
    }
}

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

        if let VMMapAddr::Fixed(addr) = addr {
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

        // After initializing, we can safely add the new subrange
        self.sub_ranges.insert(insert_idx, new_subrange);

        Ok(new_subrange_addr)
    }

    pub fn mremap(&mut self, options: &VMRemapOptions) -> Result<usize> {
        let old_addr = options.old_addr();
        let old_size = options.old_size();
        let new_size = options.new_size();
        let (vm_subrange, idx) = {
            let idx = self.find_mmap_region_idx(old_addr)?;
            let vm_subrange = self.sub_ranges[idx];
            if (vm_subrange.end() - old_addr < old_size) {
                // Across the vm range
                return_errno!(EFAULT, "can not remap across vm range");
            } else if (vm_subrange.end() - old_addr == old_size) {
                // Exactly the vm range
                (vm_subrange, idx)
            } else {
                // Part of the vm range
                let old_subrange = VMRange::new(old_addr, old_addr + old_size)?;
                let (subranges, offset) = {
                    let mut subranges = vm_subrange.subtract(&old_subrange);
                    let idx = subranges
                        .iter()
                        .position(|subrange| old_subrange.start() < subrange.start())
                        .unwrap_or_else(|| subranges.len());
                    subranges.insert(idx, old_subrange);
                    (subranges, idx)
                };
                self.sub_ranges.splice(idx..=idx, subranges.iter().cloned());
                (old_subrange, idx + offset)
            }
        };
        // Remap with a fixed new_addr, move it to new_addr
        if let Some(new_addr) = options.new_addr() {
            let new_subrange = VMRange::new(new_addr, new_addr + new_size)?;
            if vm_subrange.overlap_with(&new_subrange) {
                return_errno!(EINVAL, "old/new vm range overlap");
            }
            let new_addr = VMMapAddr::Fixed(new_addr);
            let (insert_idx, free_subrange) = self.find_free_subrange(new_size, new_addr)?;
            let new_subrange = self.alloc_subrange_from(new_size, new_addr, &free_subrange);
            return self.move_mmap_region(&vm_subrange, (insert_idx, &new_subrange));
        }
        // Remap without a fixed new_addr
        if old_size > new_size {
            // Shrink the mmap range, just unmap the useless range
            self.munmap(old_addr + new_size, old_size - new_size)?;
            Ok(old_addr)
        } else if old_size == new_size {
            // Same size, do nothing
            Ok(old_addr)
        } else {
            // Need to expand the mmap range, check if we can expand it
            if let Some(next_subrange) = self.sub_ranges.get(idx + 1) {
                let expand_size = new_size - old_size;
                if next_subrange.start() - vm_subrange.end() >= expand_size {
                    // Memory between subranges is enough, resize it
                    let vm_subrange = self.sub_ranges.get_mut(idx).unwrap();
                    vm_subrange.resize(new_size);
                    return Ok(vm_subrange.start());
                }
            }
            // Not enough memory to expand, must move it to a new place
            if !options.flags().contains(MRemapFlags::MREMAP_MAYMOVE) {
                return_errno!(ENOMEM, "not enough memory to expand");
            }
            let new_addr = VMMapAddr::Any;
            let (insert_idx, free_subrange) = self.find_free_subrange(new_size, new_addr)?;
            let new_subrange = self.alloc_subrange_from(new_size, new_addr, &free_subrange);
            self.move_mmap_region(&vm_subrange, (insert_idx, &new_subrange))
        }
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
                if subrange.size() > 0 {
                    subrange.subtract(&munmap_range)
                } else {
                    // Keep the two sentry subranges intact
                    vec![*subrange]
                }
            })
            .collect();
        self.sub_ranges = new_sub_ranges;
        Ok(())
    }

    pub fn find_mmap_region(&self, addr: usize) -> Result<&VMRange> {
        self.sub_ranges
            .iter()
            .find(|subrange| subrange.contains(addr))
            .ok_or_else(|| errno!(ESRCH, "no mmap regions that contains the address"))
    }

    fn find_mmap_region_idx(&self, addr: usize) -> Result<usize> {
        self.sub_ranges
            .iter()
            .position(|subrange| subrange.contains(addr))
            .ok_or_else(|| errno!(ESRCH, "no mmap regions that contains the address"))
    }

    fn move_mmap_region(
        &mut self,
        src_subrange: &VMRange,
        dst_idx_and_subrange: (usize, &VMRange),
    ) -> Result<usize> {
        let dst_idx = dst_idx_and_subrange.0;
        let dst_subrange = dst_idx_and_subrange.1;
        unsafe {
            let src_buf = src_subrange.as_slice_mut();
            let dst_buf = dst_subrange.as_slice_mut();
            for (d, s) in dst_buf.iter_mut().zip(src_buf.iter()) {
                *d = *s;
            }
        }
        self.sub_ranges.insert(dst_idx, *dst_subrange);
        self.munmap(src_subrange.start(), src_subrange.size())?;
        Ok(dst_subrange.start())
    }

    // Find the free subrange that satisfies the constraints of size and address
    fn find_free_subrange(&mut self, size: usize, addr: VMMapAddr) -> Result<(usize, VMRange)> {
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
                VMMapAddr::Fixed(addr) => {
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
        if let VMMapAddr::Fixed(addr) = addr {
            debug_assert!(addr == new_subrange.start());
        }

        new_subrange.resize(size);
        new_subrange
    }
}
