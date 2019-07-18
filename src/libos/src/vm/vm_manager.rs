use super::*;
use std::{slice};

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
    pub fn initialize(&self, buf: &mut [u8]) -> Result<(), Error> {
        match self {
            VMInitializer::DoNothing() => {
                // Do nothing
            },
            VMInitializer::FillZeros() => {
                for b in buf {
                    *b = 0;
                }
            },
            VMInitializer::LoadFromFile { file, offset } => {
                // TODO: make sure that read_at does not move file cursor
                let len = file.read_at(*offset, buf)?;
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
    Any,           // Free to choose any address
    Hint(usize),   // Prefer the given address
    Fixed(usize),  // Must be the given address
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
    initializer: VMInitializer
}

// VMMapOptionsBuilder is generated automatically, except the build function
impl VMMapOptionsBuilder {
    pub fn build(&self) -> Result<VMMapOptions, Error> {
        let size = {
            let size = self.size.ok_or_else(|| (Errno::EINVAL, "Invalid size for mmap"))?;
            if size == 0 {
                return errno!(EINVAL, "Invalid size for mmap");
            }
            align_up(size, PAGE_SIZE)
        };
        let align = {
            let align = self.align.unwrap_or(PAGE_SIZE);
            if align == 0 || align % PAGE_SIZE != 0 {
                return errno!(EINVAL, "Invalid size for mmap");
            }
            align
        };
        let addr = {
            let addr = self.addr.unwrap_or_default();
            match addr {
                // TODO: check addr + size overflow
                VMMapAddr::Any => {
                   VMMapAddr::Any
                }
                VMMapAddr::Hint(addr) => {
                    let addr = align_down(addr, PAGE_SIZE);
                    VMMapAddr::Hint(addr)
                }
                VMMapAddr::Fixed(addr) => {
                    if addr % align != 0 {
                        return errno!(EINVAL, "Unaligned addr for fixed mmap");
                    }
                    VMMapAddr::Fixed(addr)
                }
            }
        };
        let initializer = match self.initializer.as_ref() {
            Some(initializer) => { initializer.clone() }
            None => { VMInitializer::default() }
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


#[derive(Debug, Default)]
pub struct VMManager {
    range: VMRange,
    sub_ranges: Vec<VMRange>,
}

impl VMManager {
    pub fn from(addr: usize, size: usize) -> Result<VMManager, Error> {
        let range = VMRange::from(addr, addr + size)?;
        let sub_ranges = {
            let start = range.start();
            let end = range.end();
            let start_sentry = VMRange::from(start, start)?;
            let end_sentry = VMRange::from(end, end)?;
            vec![start_sentry, end_sentry]
        };
        Ok(VMManager {
            range,
            sub_ranges,
        })
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn mmap(
        &mut self,
        options: &VMMapOptions,
    ) -> Result<usize, Error> {
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
            let buf_ptr = new_subrange.start() as *mut u8;
            let buf_size = new_subrange.size() as usize;
            let buf = slice::from_raw_parts_mut(buf_ptr, buf_size);
            options.initializer.initialize(buf)?;
        }

        // After initializing, we can safely add the new subrange
        self.sub_ranges.insert(insert_idx, new_subrange);

        Ok(new_subrange_addr)
    }

    pub fn munmap(&mut self, addr: usize, size: usize) -> Result<(), Error> {
        let size = {
            if size == 0 {
                return errno!(EINVAL, "size of munmap must not be zero");
            }
            align_up(size, PAGE_SIZE)
        };
        let munmap_range = {
            let munmap_range = VMRange::from(addr, addr + size)?;

            let effective_munmap_range_opt = munmap_range.intersect(&self.range);
            if effective_munmap_range_opt.is_none() {
                return Ok(())
            }

            let effective_munmap_range = effective_munmap_range_opt.unwrap();
            if effective_munmap_range.empty() {
                return Ok(())
            }
            effective_munmap_range
        };

        let new_sub_ranges = self.sub_ranges
            .iter()
            .flat_map(|subrange| {
                if subrange.size() > 0 {
                    subrange.subtract(&munmap_range)
                } else { // Keep the two sentry subranges intact
                    vec![*subrange]
                }
            })
            .collect();
        self.sub_ranges = new_sub_ranges;
        Ok(())
    }

    pub fn find_mmap_region(&self, addr: usize) -> Result<&VMRange, Error> {
        self.sub_ranges.iter()
            .find(|subrange| subrange.contains(addr))
            .ok_or(Error::new(Errno::ESRCH,
                              "no mmap regions that contains the address"))
    }

    // Find the free subrange that satisfies the constraints of size and address
    fn find_free_subrange(
        &mut self,
        size: usize,
        addr: VMMapAddr
    ) -> Result<(usize, VMRange), Error> {
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

                unsafe {
                    VMRange::from_unchecked(free_range_start, free_range_end)
                }
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
                        return errno!(ENOMEM, "Not enough memory for fixed mmap");
                    }
                    if !free_range.contains(addr) {
                        continue;
                    }
                    if free_range.end() - addr < size {
                        return errno!(ENOMEM, "Not enough memory for fixed mmap");
                    }
                    free_range.start = addr;
                    let insert_idx = idx + 1;
                    return Ok((insert_idx, free_range));
                }
            }

            if result_free_range == None
                || result_free_range.as_ref().unwrap().size() > free_range.size() {
                result_free_range = Some(free_range);
                result_idx = Some(idx);
            }
        }

        if result_free_range.is_none() {
            return errno!(ENOMEM, "Cannot find enough memory");
        }

        let free_range = result_free_range.unwrap();
        let insert_idx = result_idx.unwrap() + 1;
        Ok((insert_idx, free_range))
    }

    fn alloc_subrange_from(
        &self,
        size: usize,
        addr: VMMapAddr,
        free_subrange: &VMRange
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

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct VMRange {
    start: usize,
    end: usize,
}

impl VMRange {
    pub fn from(start: usize, end: usize) -> Result<VMRange, Error> {
        if start % PAGE_SIZE != 0 || end % PAGE_SIZE != 0 || start > end {
            return errno!(EINVAL, "invalid start or end");
        }
        Ok(VMRange {
            start: start,
            end: end,
        })
    }

    pub unsafe fn from_unchecked(start: usize, end: usize) -> VMRange {
        debug_assert!(start % PAGE_SIZE == 0);
        debug_assert!(end % PAGE_SIZE == 0);
        debug_assert!(start <= end);
        VMRange {
            start: start,
            end: end,
        }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn size(&self) -> usize {
        self.end - self.start
    }

    pub fn resize(&mut self, new_size: usize) {
        self.end = self.start + new_size;
    }

    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    pub fn is_superset_of(&self, other: &VMRange) -> bool {
        self.start() <= other.start() && other.end() <= self.end()
    }

    pub fn contains(&self, addr: usize) -> bool {
        self.start() <= addr && addr < self.end()
    }

    pub fn subtract(&self, other: &VMRange) -> Vec<VMRange> {
        let self_start = self.start();
        let self_end = self.end();
        let other_start = other.start();
        let other_end = other.end();

        match (self_start < other_start, other_end < self_end) {
            (false, false) => {
                Vec::new()
            }
            (false, true) => unsafe {
                vec![VMRange::from_unchecked(self_start.max(other_end), self_end)]
            }
            (true, false) => unsafe {
                vec![VMRange::from_unchecked(self_start, self_end.min(other_start))]
            }
            (true, true) => unsafe {
                vec![VMRange::from_unchecked(self_start, other_start),
                     VMRange::from_unchecked(other_end, self_end)]
            }
        }
    }

    pub fn intersect(&self, other: &VMRange) -> Option<VMRange> {
        let intersection_start = self.start().max(other.start());
        let intersection_end = self.end().min(other.end());
        if intersection_start > intersection_end {
            return None;
        }
        unsafe {
            Some(VMRange::from_unchecked(intersection_start, intersection_end))
        }
    }
}
