use super::*;
use std::{fmt};

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
    }
}

impl_vmrange_trait_for!(VMRange, inner);
impl_vmrange_trait_for!(VMSpace, range);
impl_vmrange_trait_for!(VMDomain, range);
impl_vmrange_trait_for!(VMArea, range);


#[derive(Debug)]
pub struct VMRange {
    inner: VMRangeInner,
    parent_range: *const VMRange,
    sub_ranges: Option<Vec<VMRangeInner>>,
}

impl VMRange {
    pub unsafe fn new(start: usize, end: usize, growth: VMGrowthType) -> Result<VMRange, Error> {
        if start % PAGE_SIZE != 0 || end % PAGE_SIZE != 0 {
            return errno!(EINVAL, "Invalid start and/or end");
        }
        Ok(VMRange {
            inner: VMRangeInner::new(start, end, growth),
            parent_range: 0 as *const VMRange,
            sub_ranges: None,
        })
    }

    pub fn alloc_subrange(&mut self, options: &VMAllocOptions) -> Result<VMRange, Error> {
        // Get valid parameters from options
        let size = options.size;
        let addr = options.addr;
        let growth = options.growth.unwrap_or(VMGrowthType::Fixed);

        // Lazy initialize the subrange array upon the first allocation
        if !self.has_subranges() {
            self.init_subranges()?;
        }

        // Find a free space for allocating a VMRange
        let free_space = {
            // Look for the minimal big-enough free space
            let mut min_big_enough_free_space : Option<FreeSpace> = None;
            let sub_ranges = self.get_subranges();
            for (idx, range_pair) in sub_ranges.windows(2).enumerate() {
                let pre_range = &range_pair[0];
                let next_range = &range_pair[1];

                let mut free_range = {
                    let free_range_start = pre_range.get_end();
                    let free_range_end = next_range.get_start();

                    let free_range_size = free_range_end - free_range_start;
                    if free_range_size < size { continue }

                    free_range_start..free_range_end
                };

                match addr {
                    VMAddrOption::Hint(addr) | VMAddrOption::Fixed(addr) => {
                        if !free_range.contains(&addr) { continue }
                        free_range.start = addr;
                    }
                    VMAddrOption::Beyond(addr) => {
                        if free_range.start < addr { continue }
                    }
                    _ => {}
                }

                let free_space = Some(FreeSpace {
                    index_in_subranges: idx + 1,
                    start: free_range.start,
                    end: free_range.end,
                    may_neighbor_grow: (pre_range.growth == VMGrowthType::Upward,
                                        next_range.growth == VMGrowthType::Downward),
                });

                if min_big_enough_free_space == None ||
                    free_space < min_big_enough_free_space
                {
                    min_big_enough_free_space = free_space;

                    match addr {
                        VMAddrOption::Hint(addr) | VMAddrOption::Fixed(addr) => {
                            break
                        }
                        _ => {},
                    }
                }
            }

            if min_big_enough_free_space.is_none() {
                return errno!(ENOMEM, "No enough space");
            }
            min_big_enough_free_space.unwrap()
        };

        // Given the free space, determine the start and end of the sub-range
        let (new_subrange_start, new_subrange_end) = match addr {
            VMAddrOption::Any | VMAddrOption::Beyond(_) => {
                let should_no_gap_to_pre_domain =
                    free_space.may_neighbor_grow.0 == false &&
                    growth != VMGrowthType::Downward;
                let should_no_gap_to_next_domain =
                    free_space.may_neighbor_grow.1 == false &&
                    growth != VMGrowthType::Upward;
                let domain_start = if should_no_gap_to_pre_domain {
                    free_space.start
                }
                else if should_no_gap_to_next_domain {
                    free_space.end - size
                }
                else {
                    // We want to leave some space at both ends in case
                    // this sub-range or neighbor sub-range needs to grow later.
                    // As a simple heuristic, we put this sub-range near the
                    // center between the previous and next sub-ranges.
                    free_space.start + (free_space.get_size() - size) / 2
                };
                (domain_start, domain_start + size)
            }
            VMAddrOption::Fixed(addr) => {
                (addr, addr + size)
            }
            VMAddrOption::Hint(addr) => {
                return errno!(EINVAL, "Not implemented");
            }
        };

        let new_subrange_inner = VMRangeInner::new(new_subrange_start,
            new_subrange_end, growth);
        self.get_subranges_mut().insert(free_space.index_in_subranges,
                                        new_subrange_inner);
        // Although there are two copies of the newly created VMRangeInner obj,
        // we can keep them in sync as all mutation on VMRange object must
        // be carried out through dealloc_subrange() and resize_subrange() that
        // takes both a (parent) range and its (child) sub-range as parameters.
        // We update both copies of VMRangeInner, one in parent and the
        // other in child, in dealloc_subrange and resize_subrange functions.
        Ok(VMRange {
            inner: new_subrange_inner,
            parent_range: self as *const VMRange,
            sub_ranges: None,
        })
    }

    pub fn dealloc_subrange(&mut self, subrange: &mut VMRange) {
        self.ensure_subrange_is_a_child(subrange);
        if subrange.has_subranges() {
            panic!("A range can only be dealloc'ed when it has no sub-ranges");
        }

        // Remove the sub-range
        let domain_i = self.position_subrange(subrange);
        self.get_subranges_mut().remove(domain_i);

        // When all sub-ranges are removed, remove the sub-range array
        if self.get_subranges().len() == 2 { // two sentinel sub-ranges excluded
            self.sub_ranges = None;
        }

        // Mark a range as dealloc'ed
        subrange.mark_as_dealloced();
    }

    pub fn resize_subrange(&mut self, subrange: &mut VMRange, options: &VMResizeOptions)
        -> Result<(), Error> {
        self.ensure_subrange_is_a_child(subrange);

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
            self.grow_subrange_to(subrange, new_size)
        }
    }

    fn init_subranges(&mut self) -> Result<(), Error> {
        // Use dummy VMRange as sentinel object at both ends to make the allocation
        // and deallocation algorithm simpler
        let start = self.get_start();
        let end = self.get_end();
        let start_sentry = VMRangeInner::new(start, start, VMGrowthType::Fixed);
        let end_sentry = VMRangeInner::new(end, end, VMGrowthType::Fixed);
        self.sub_ranges = Some(vec![start_sentry, end_sentry]);
        Ok(())
    }

    fn ensure_subrange_is_a_child(&self, subrange: &VMRange) {
        // FIXME:
        /*if subrange.parent_range != self as *const VMRange {
            panic!("This range does not contain the given sub-range");
        }*/
    }

    fn position_subrange(&self, subrange: &VMRange) -> usize {
        let sub_ranges = self.get_subranges();
        sub_ranges.iter().position(|d| d == &subrange.inner).unwrap()
    }

    fn get_subranges(&self) -> &Vec<VMRangeInner> {
        self.sub_ranges.as_ref().unwrap()
    }

    fn get_subranges_mut(&mut self) -> &mut Vec<VMRangeInner> {
        self.sub_ranges.as_mut().unwrap()
    }

    fn has_subranges(&self) -> bool {
        self.sub_ranges.is_some()
    }

    fn shrink_subrange_to(&mut self, subrange: &mut VMRange, new_size: usize)
        -> Result<(), Error>
    {
        let subrange_i = self.position_subrange(subrange);
        let subranges = self.get_subranges_mut();

        if subrange.inner.growth == VMGrowthType::Upward {
            // Can we do shrink?
            let min_new_size = match subrange.sub_ranges.as_mut() {
                Some(child_subranges) => {
                    let child_last_subrange = &child_subranges[
                        child_subranges.len() - 2];
                    child_last_subrange.end - subrange.inner.start
                }
                None => {
                    0
                }
            };
            if new_size < min_new_size {
                return errno!(ENOMEM, "Cannot shrink to new size");
            }
            // Do shrink
            let new_subrange_end = subrange.inner.start + new_size;
            subrange.inner.end = new_subrange_end;
            // Sync state
            subranges[subrange_i].end = new_subrange_end;
        }
        else { // self.growth == VMGrowthType::Downward
            // Can we do shrink?
            let min_new_size = match subrange.sub_ranges.as_mut() {
                Some(child_subranges) => {
                    let child_first_subrange = &child_subranges[1];
                    subrange.inner.end - child_first_subrange.start
                }
                None => {
                    0
                }
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

    fn grow_subrange_to(&mut self, subrange: &mut VMRange, new_size: usize)
        -> Result<(), Error>
    {
        let subrange_i = self.position_subrange(subrange);
        let subranges = self.get_subranges_mut();

        if subrange.inner.growth == VMGrowthType::Upward {
            // Can we grow?
            let max_new_size = {
                let next_subrange = &subranges[subrange_i + 1];
                next_subrange.start - subrange.inner.start
            };
            if new_size > max_new_size {
                return errno!(ENOMEM, "Cannot grow to new size");
            }
            // Do grow
            let subrange_new_end = subrange.inner.start + new_size;
            subrange.inner.end = subrange_new_end;
            // Sync state
            subranges[subrange_i].end = subrange_new_end;
        }
        else { // self.growth == VMGrowthType::Downward
            // Can we grow?
            let max_new_size = {
                let pre_subrange = &subranges[subrange_i - 1];
                subrange.inner.end - pre_subrange.end
            };
            if new_size > max_new_size {
                return errno!(ENOMEM, "Cannot grow to new size");
            }
            // Do grow
            let subrange_new_start = subrange.inner.end - new_size;
            subrange.inner.start = subrange_new_start;
            // Sync state
            subranges[subrange_i].start = subrange_new_start;
        }
        Ok(())
    }

    fn mark_as_dealloced(&mut self) {
        self.parent_range = 0 as *const VMRange;
        self.inner.start = self.inner.end;
    }

    fn is_dealloced(&self) -> bool {
        self.parent_range == 0 as *const VMRange
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
        if !self.is_dealloced() {
            panic!("A range must be dealloc'ed before drop");
        }
        if self.has_subranges() {
            panic!("All sub-ranges must be removed explicitly before drop");
        }
    }
}

unsafe impl Send for VMRange {}
unsafe impl Sync for VMRange {}

impl Default for VMRange {
    fn default() -> VMRange {
        VMRange {
            inner: VMRangeInner::new(0, 0, VMGrowthType::Fixed),
            parent_range: 0 as *const VMRange,
            sub_ranges: None,
        }
    }
}


#[derive(Clone, Copy)]
pub struct VMRangeInner {
    start: usize,
    end: usize,
    growth: VMGrowthType,
}

impl VMRangeInner {
    pub fn new(start: usize, end: usize, growth: VMGrowthType) -> VMRangeInner
    {
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
        write!(f, "VMRangeInner {{ start: 0x{:X?}, end: 0x{:X?}, size: 0x{:X?}, growth: {:?} }}",
               self.start, self.end, self.get_size(), self.growth)
    }
}

impl PartialOrd for VMRangeInner {
    fn partial_cmp(&self, other: &VMRangeInner) -> Option<Ordering> {
        if self.end <= other.start {
            return Some(Ordering::Less);
        }
        else if self.start >= other.end {
            return Some(Ordering::Greater);
        }
        else if self.start == other.start && self.end == other.end {
            return Some(Ordering::Equal);
        }
        else {
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
}

impl PartialEq for FreeSpace {
    fn eq(&self, other: &FreeSpace) -> bool {
        self.get_size() == other.get_size() &&
            self.get_neighbor_pressure() == other.get_neighbor_pressure()
    }
}

impl PartialOrd for FreeSpace {
    fn partial_cmp(&self, other: &FreeSpace) -> Option<Ordering> {
        let self_size = self.get_size();
        let other_size = other.get_size();
        if self_size < other_size {
            Some(Ordering::Less)
        }
        else if self_size > other_size {
            Some(Ordering::Greater)
        }
        else {
            // The less neighbor pressure, the larger the free space
            let self_neighbor_pressure = self.get_neighbor_pressure();
            let other_neighbor_pressure = other.get_neighbor_pressure();
            if self_neighbor_pressure > other_neighbor_pressure {
                Some(Ordering::Less)
            }
            else if self_neighbor_pressure < other_neighbor_pressure {
                Some(Ordering::Greater)
            }
            else {
                Some(Ordering::Equal)
            }
        }
    }
}
