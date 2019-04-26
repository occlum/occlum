use super::*;
use std::fmt;

pub trait VMRangeTrait {
    fn get_start(&self) -> usize;
    fn get_end(&self) -> usize;
    fn get_size(&self) -> usize;
    fn get_growth(&self) -> VMGrowthType;
    fn contains_obj(&self, ptr: usize, size: usize) -> bool;
}

macro_rules! impl_vmrange_trait_for {
    ($struct_name: ident, $field: ident) => {
        impl VMRangeTrait for $struct_name {
            fn get_start(&self) -> usize {
                self.$field.get_start()
            }

            fn get_end(&self) -> usize {
                self.$field.get_end()
            }

            fn get_size(&self) -> usize {
                self.$field.get_end() - self.$field.get_start()
            }

            fn get_growth(&self) -> VMGrowthType {
                self.$field.get_growth()
            }

            fn contains_obj(&self, ptr: usize, size: usize) -> bool {
                self.$field.contains_obj(ptr, size)
            }
        }
    };
}

#[derive(Debug)]
pub struct VMRange {
    inner: VMRangeInner,
    sub_ranges: Option<Vec<VMRangeInner>>,
    is_dealloced: bool,
    description: String,
}

impl_vmrange_trait_for!(VMRange, inner);

impl VMRange {
    pub unsafe fn new(
        start: usize,
        end: usize,
        growth: VMGrowthType,
        description: &str,
    ) -> Result<VMRange, Error> {
        if start % PAGE_SIZE != 0 || end % PAGE_SIZE != 0 {
            return errno!(EINVAL, "Invalid start and/or end");
        }
        Ok(VMRange {
            inner: VMRangeInner::new(start, end, growth),
            sub_ranges: None,
            is_dealloced: false,
            description: description.to_owned(),
        })
    }

    pub fn alloc_subrange(&mut self, options: &VMAllocOptions) -> Result<VMRange, Error> {
        debug_assert!(!self.is_dealloced);

        // Lazy initialize the subrange array upon the first allocation
        if self.sub_ranges.is_none() {
            self.init_subrange_array()?;
        }

        // Find a free space that satisfies the options
        let free_space = self.look_for_free_space(options)?;
        // Allocate a new subrange from the free space
        let (new_subrange_idx, new_subrange_inner) = {
            let (new_subrange_start, new_subrange_end) =
                self.alloc_from_free_space(&free_space, options);
            debug_assert!(free_space.contains(new_subrange_start));
            debug_assert!(free_space.contains(new_subrange_end));

            (
                free_space.index_in_subranges,
                VMRangeInner::new(new_subrange_start, new_subrange_end, options.growth),
            )
        };
        self.get_subranges_mut()
            .insert(new_subrange_idx, new_subrange_inner);

        if options.fill_zeros {
            // Init the memory area with all zeros
            unsafe {
                let mem_ptr = new_subrange_inner.get_start() as *mut c_void;
                let mem_size = new_subrange_inner.get_size() as size_t;
                memset(mem_ptr, 0 as c_int, mem_size);
            }
        }

        // Although there are two copies of the newly created VMRangeInner obj,
        // we can keep them in sync as all mutation on VMRange object must
        // be carried out through dealloc_subrange() and resize_subrange() that
        // takes both a (parent) range and its (child) sub-range as parameters.
        // We update both copies of VMRangeInner, one in parent and the
        // other in child, in dealloc_subrange and resize_subrange functions.
        Ok(VMRange {
            inner: new_subrange_inner,
            sub_ranges: None,
            is_dealloced: false,
            description: options.description.clone(),
        })
    }

    pub fn dealloc_subrange(&mut self, subrange: &mut VMRange) {
        debug_assert!(!self.is_dealloced);
        debug_assert!(!subrange.is_dealloced);
        debug_assert!(self.sub_ranges.is_some());

        // Remove the sub-range
        let domain_i = self.position_subrange(subrange);
        self.get_subranges_mut().remove(domain_i);
        // When all sub-ranges are removed, remove the sub-range array
        if self.get_subranges().len() == 2 {
            // two sentinel sub-ranges excluded
            self.sub_ranges = None;
        }

        subrange.inner.end = subrange.inner.start;
        subrange.is_dealloced = true;
    }

    pub fn resize_subrange(
        &mut self,
        subrange: &mut VMRange,
        options: &VMResizeOptions,
    ) -> Result<(), Error> {
        debug_assert!(!self.is_dealloced);
        debug_assert!(!subrange.is_dealloced);
        debug_assert!(self.sub_ranges.is_some());

        // Get valid parameters from options
        let new_size = options.new_size;
        let new_addr = options.new_addr;

        // Handle no-resizing cases
        if subrange.get_size() == new_size {
            return Ok(());
        }
        if subrange.get_growth() == VMGrowthType::Fixed {
            return errno!(EINVAL, "Cannot resize a fixed range");
        }

        // Shrink
        if new_size < subrange.get_size() {
            self.shrink_subrange_to(subrange, new_size)
        }
        // Grow
        else {
            self.grow_subrange_to(subrange, new_size, options.fill_zeros)
        }
    }

    pub fn get_description(&self) -> &str {
        &self.description
    }

    fn init_subrange_array(&mut self) -> Result<(), Error> {
        // Use dummy VMRange as sentinel object at both ends to make the allocation
        // and deallocation algorithm simpler
        let start = self.get_start();
        let end = self.get_end();
        let start_sentry = VMRangeInner::new(start, start, VMGrowthType::Fixed);
        let end_sentry = VMRangeInner::new(end, end, VMGrowthType::Fixed);
        self.sub_ranges = Some(vec![start_sentry, end_sentry]);
        Ok(())
    }

    // Find a free space for allocating a sub VMRange
    fn look_for_free_space(&mut self, options: &VMAllocOptions) -> Result<FreeSpace, Error> {
        // TODO: reduce the complexity from O(N) to O(log(N)), where N is
        // the number of existing subranges.

        // Get valid parameters from options
        let size = options.size;
        let addr = options.addr;
        let growth = options.growth;

        // Record the minimal free space that satisfies the options
        let mut min_big_enough_free_space: Option<FreeSpace> = None;

        let sub_ranges = self.get_subranges();
        for (idx, range_pair) in sub_ranges.windows(2).enumerate() {
            let pre_range = &range_pair[0];
            let next_range = &range_pair[1];

            let (free_range_start, free_range_end) = {
                let free_range_start = pre_range.get_end();
                let free_range_end = next_range.get_start();

                let free_range_size = free_range_end - free_range_start;
                if free_range_size < size {
                    continue;
                }

                (free_range_start, free_range_end)
            };
            let mut free_space = FreeSpace {
                index_in_subranges: idx + 1,
                start: free_range_start,
                end: free_range_end,
                may_neighbor_grow: (
                    pre_range.growth == VMGrowthType::Upward,
                    next_range.growth == VMGrowthType::Downward,
                ),
            };

            match addr {
                // Want a minimal free_space
                VMAddrOption::Any => {}
                // Prefer to have free_space.start == addr
                VMAddrOption::Hint(addr) => {
                    if free_space.contains(addr) {
                        if free_space.end - addr >= size {
                            free_space.start = addr;
                            return Ok(free_space);
                        }
                    }
                }
                // Must have free_space.start == addr
                VMAddrOption::Fixed(addr) => {
                    if !free_space.contains(addr) {
                        continue;
                    }
                    if free_space.end - addr < size {
                        return errno!(ENOMEM, "not enough memory");
                    }
                    free_space.start = addr;
                    return Ok(free_space);
                }
                // Must have free_space.start >= addr
                VMAddrOption::Beyond(addr) => {
                    if free_space.end < addr {
                        continue;
                    }
                    if free_space.contains(addr) {
                        free_space.start = addr;
                        if free_space.get_size() < size {
                            continue;
                        }
                    }
                }
            }

            if min_big_enough_free_space == None
                || free_space < *min_big_enough_free_space.as_ref().unwrap()
            {
                min_big_enough_free_space = Some(free_space);
            }
        }

        min_big_enough_free_space.ok_or_else(|| Error::new(Errno::ENOMEM, "not enough space"))
    }

    fn alloc_from_free_space(
        &self,
        free_space: &FreeSpace,
        options: &VMAllocOptions,
    ) -> (usize, usize) {
        // Get valid parameters from options
        let size = options.size;
        let addr_option = options.addr;
        let growth = options.growth;

        if let VMAddrOption::Fixed(addr) = addr_option {
            return (addr, addr + size);
        } else if let VMAddrOption::Hint(addr) = addr_option {
            if free_space.start == addr {
                return (addr, addr + size);
            }
        }

        let should_no_gap_to_pre_domain =
            free_space.may_neighbor_grow.0 == false && growth != VMGrowthType::Downward;
        let should_no_gap_to_next_domain =
            free_space.may_neighbor_grow.1 == false && growth != VMGrowthType::Upward;

        let addr = if should_no_gap_to_pre_domain {
            free_space.start
        } else if should_no_gap_to_next_domain {
            free_space.end - size
        } else {
            // We want to leave some space at both ends in case
            // this sub-range or neighbor sub-range needs to grow later.
            // As a simple heuristic, we put this sub-range near the
            // center between the previous and next sub-ranges.
            let offset = align_down((free_space.get_size() - size) / 2, PAGE_SIZE);
            free_space.start + offset
        };

        (addr, addr + size)
    }

    fn position_subrange(&self, subrange: &VMRange) -> usize {
        let sub_ranges = self.get_subranges();
        sub_ranges
            .iter()
            .position(|d| d == &subrange.inner)
            .unwrap()
    }

    fn get_subranges(&self) -> &Vec<VMRangeInner> {
        self.sub_ranges.as_ref().unwrap()
    }

    fn get_subranges_mut(&mut self) -> &mut Vec<VMRangeInner> {
        self.sub_ranges.as_mut().unwrap()
    }

    fn shrink_subrange_to(&mut self, subrange: &mut VMRange, new_size: usize) -> Result<(), Error> {
        let subrange_i = self.position_subrange(subrange);
        let subranges = self.get_subranges_mut();

        if subrange.inner.growth == VMGrowthType::Upward {
            // Can we do shrink?
            let min_new_size = match subrange.sub_ranges.as_mut() {
                Some(child_subranges) => {
                    let child_last_subrange = &child_subranges[child_subranges.len() - 2];
                    child_last_subrange.end - subrange.inner.start
                }
                None => 0,
            };
            if new_size < min_new_size {
                return errno!(ENOMEM, "Cannot shrink to new size");
            }
            // Do shrink
            let new_subrange_end = subrange.inner.start + new_size;
            subrange.inner.end = new_subrange_end;
            // Sync state
            subranges[subrange_i].end = new_subrange_end;
        } else {
            // self.growth == VMGrowthType::Downward
            // Can we do shrink?
            let min_new_size = match subrange.sub_ranges.as_mut() {
                Some(child_subranges) => {
                    let child_first_subrange = &child_subranges[1];
                    subrange.inner.end - child_first_subrange.start
                }
                None => 0,
            };
            if new_size < min_new_size {
                return errno!(ENOMEM, "Cannot shrink to new size");
            }
            // Do shrink
            let new_subrange_start = subrange.inner.end - new_size;
            subrange.inner.start = new_subrange_start;
            // Sync state
            subranges[subrange_i].start = new_subrange_start;
        }
        Ok(())
    }

    fn grow_subrange_to(
        &mut self,
        subrange: &mut VMRange,
        new_size: usize,
        fill_zeros: bool,
    ) -> Result<(), Error> {
        let subrange_i = self.position_subrange(subrange);
        let subranges = self.get_subranges_mut();

        let subrange_old_start = subrange.inner.start;
        let subrange_old_end = subrange.inner.end;
        let subrange_old_size = subrange.get_size();

        if subrange.inner.growth == VMGrowthType::Upward {
            // Can we grow upward?
            let max_new_size = {
                let next_subrange = &subranges[subrange_i + 1];
                next_subrange.start - subrange_old_start
            };
            if new_size > max_new_size {
                return errno!(ENOMEM, "Cannot grow to new size");
            }
            // Do grow
            let subrange_new_end = subrange_old_start + new_size;
            subrange.inner.end = subrange_new_end;
            // Sync state
            subranges[subrange_i].end = subrange_new_end;
            // Init memory
            if fill_zeros {
                unsafe {
                    let mem_ptr = subrange_old_end as *mut c_void;
                    let mem_size = (subrange_new_end - subrange_old_end) as size_t;
                    memset(mem_ptr, 0 as c_int, mem_size);
                }
            }
        } else {
            // self.growth == VMGrowthType::Downward
            // Can we grow downard?
            let max_new_size = {
                let pre_subrange = &subranges[subrange_i - 1];
                subrange_old_end - pre_subrange.end
            };
            if new_size > max_new_size {
                return errno!(ENOMEM, "Cannot grow to new size");
            }
            // Do grow
            let subrange_new_start = subrange_old_end - new_size;
            subrange.inner.start = subrange_new_start;
            // Sync state
            subranges[subrange_i].start = subrange_new_start;
            // Init memory
            if fill_zeros {
                unsafe {
                    let mem_ptr = subrange_new_start as *mut c_void;
                    let mem_size = (subrange_old_start - subrange_new_start) as size_t;
                    memset(mem_ptr, 0 as c_int, mem_size);
                }
            }
        }
        Ok(())
    }
}

impl PartialOrd for VMRange {
    fn partial_cmp(&self, other: &VMRange) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl PartialEq for VMRange {
    fn eq(&self, other: &VMRange) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl Drop for VMRange {
    fn drop(&mut self) {
        if !self.is_dealloced {
            panic!("A range must be dealloc'ed before drop");
        }
    }
}

unsafe impl Send for VMRange {}
unsafe impl Sync for VMRange {}

#[derive(Clone, Copy)]
pub struct VMRangeInner {
    start: usize,
    end: usize,
    growth: VMGrowthType,
}

impl VMRangeInner {
    pub fn new(start: usize, end: usize, growth: VMGrowthType) -> VMRangeInner {
        debug_assert!(start % PAGE_SIZE == 0);
        debug_assert!(end % PAGE_SIZE == 0);
        VMRangeInner {
            start: start,
            end: end,
            growth: growth,
        }
    }
}

impl VMRangeTrait for VMRangeInner {
    fn get_start(&self) -> usize {
        self.start
    }

    fn get_end(&self) -> usize {
        self.end
    }

    fn get_size(&self) -> usize {
        self.end - self.start
    }

    fn get_growth(&self) -> VMGrowthType {
        self.growth
    }

    fn contains_obj(&self, ptr: usize, size: usize) -> bool {
        let obj_begin = ptr as usize;
        let obj_end = obj_begin + size;
        self.start <= obj_begin && obj_end < self.end
    }
}

impl fmt::Debug for VMRangeInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "VMRangeInner {{ start: 0x{:X?}, end: 0x{:X?}, size: 0x{:X?}, growth: {:?} }}",
            self.start,
            self.end,
            self.get_size(),
            self.growth
        )
    }
}

impl PartialOrd for VMRangeInner {
    fn partial_cmp(&self, other: &VMRangeInner) -> Option<Ordering> {
        if self.end <= other.start {
            return Some(Ordering::Less);
        } else if self.start >= other.end {
            return Some(Ordering::Greater);
        } else if self.start == other.start && self.end == other.end {
            return Some(Ordering::Equal);
        } else {
            return None;
        }
    }
}

impl PartialEq for VMRangeInner {
    fn eq(&self, other: &VMRangeInner) -> bool {
        self.start == other.start && self.end == other.end
    }
}

#[derive(Debug)]
struct FreeSpace {
    index_in_subranges: usize,
    start: usize,
    end: usize,
    may_neighbor_grow: (bool, bool),
}

impl FreeSpace {
    fn get_neighbor_pressure(&self) -> u32 {
        let mut pressure = 0;
        pressure += if self.may_neighbor_grow.0 { 1 } else { 0 };
        pressure += if self.may_neighbor_grow.1 { 1 } else { 0 };
        pressure
    }

    fn get_size(&self) -> usize {
        self.end - self.start
    }

    fn contains(&self, addr: usize) -> bool {
        self.start <= addr && addr < self.end
    }
}

impl PartialEq for FreeSpace {
    fn eq(&self, other: &FreeSpace) -> bool {
        self.get_size() == other.get_size()
            && self.get_neighbor_pressure() == other.get_neighbor_pressure()
    }
}

impl PartialOrd for FreeSpace {
    fn partial_cmp(&self, other: &FreeSpace) -> Option<Ordering> {
        let self_size = self.get_size();
        let other_size = other.get_size();
        if self_size < other_size {
            Some(Ordering::Less)
        } else if self_size > other_size {
            Some(Ordering::Greater)
        } else {
            // The less neighbor pressure, the larger the free space
            let self_neighbor_pressure = self.get_neighbor_pressure();
            let other_neighbor_pressure = other.get_neighbor_pressure();
            if self_neighbor_pressure > other_neighbor_pressure {
                Some(Ordering::Less)
            } else if self_neighbor_pressure < other_neighbor_pressure {
                Some(Ordering::Greater)
            } else {
                Some(Ordering::Equal)
            }
        }
    }
}

#[link(name = "sgx_tstdc")]
extern "C" {
    pub fn memset(p: *mut c_void, c: c_int, n: size_t) -> *mut c_void;
}
