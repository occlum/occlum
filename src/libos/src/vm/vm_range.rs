use super::*;

#[derive(Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct VMRange {
    pub(super) start: usize,
    pub(super) end: usize,
}

impl VMRange {
    pub fn new(start: usize, end: usize) -> Result<VMRange> {
        if start % PAGE_SIZE != 0 || end % PAGE_SIZE != 0 || start > end {
            return_errno!(EINVAL, "invalid start or end");
        }
        Ok(VMRange {
            start: start,
            end: end,
        })
    }

    pub fn new_with_size(start: usize, size: usize) -> Result<VMRange> {
        let end = start
            .checked_add(size)
            .ok_or_else(|| errno!(EINVAL, "end address overflow"))?;
        Self::new(start, end)
    }

    pub fn new_empty(start: usize) -> Result<VMRange> {
        if start % PAGE_SIZE != 0 {
            return_errno!(EINVAL, "invalid start or end");
        }
        Ok(VMRange {
            start: start,
            end: start,
        })
    }

    pub fn new_with_layout(layout: &VMLayout, min_start: usize) -> VMRange {
        let start = align_up(min_start, layout.align());
        let end = align_up(start + layout.size(), PAGE_SIZE);
        unsafe { VMRange::from_unchecked(start, end) }
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
        debug_assert!(new_size % PAGE_SIZE == 0);
        self.end = self.start + new_size;
    }

    pub fn set_start(&mut self, new_start: usize) {
        debug_assert!(new_start % PAGE_SIZE == 0 && new_start <= self.end);
        self.start = new_start;
    }

    pub fn set_end(&mut self, new_end: usize) {
        debug_assert!(new_end % PAGE_SIZE == 0 && new_end >= self.start);
        self.end = new_end;
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

    // Returns whether two ranges have non-empty intersection.
    pub fn overlap_with(&self, other: &VMRange) -> bool {
        let intersection_start = self.start().max(other.start());
        let intersection_end = self.end().min(other.end());
        intersection_start < intersection_end
    }

    // Returns a set of ranges by subtracting self with the other.
    //
    // Post-condition: the returned ranges have non-zero sizes.
    pub fn subtract(&self, other: &VMRange) -> Vec<VMRange> {
        if self.size() == 0 {
            return vec![];
        }

        let intersection = match self.intersect(other) {
            None => return vec![*self],
            Some(intersection) => intersection,
        };

        let self_start = self.start();
        let self_end = self.end();
        let inter_start = intersection.start();
        let inter_end = intersection.end();
        debug_assert!(self_start <= inter_start);
        debug_assert!(inter_end <= self_end);

        match (self_start < inter_start, inter_end < self_end) {
            (false, false) => Vec::new(),
            (false, true) => unsafe { vec![VMRange::from_unchecked(inter_end, self_end)] },
            (true, false) => unsafe { vec![VMRange::from_unchecked(self_start, inter_start)] },
            (true, true) => unsafe {
                vec![
                    VMRange::from_unchecked(self_start, inter_start),
                    VMRange::from_unchecked(inter_end, self_end),
                ]
            },
        }
    }

    // Returns an non-empty intersection if where is any
    pub fn intersect(&self, other: &VMRange) -> Option<VMRange> {
        let intersection_start = self.start().max(other.start());
        let intersection_end = self.end().min(other.end());
        if intersection_start >= intersection_end {
            return None;
        }
        unsafe {
            Some(VMRange::from_unchecked(
                intersection_start,
                intersection_end,
            ))
        }
    }

    pub fn is_contiguous_with(&self, other: &VMRange) -> bool {
        self.start == other.end || self.end == other.start
    }

    pub unsafe fn as_slice(&self) -> &[u8] {
        let buf_ptr = self.start() as *const u8;
        let buf_size = self.size() as usize;
        std::slice::from_raw_parts(buf_ptr, buf_size)
    }

    pub unsafe fn as_slice_mut(&self) -> &mut [u8] {
        let buf_ptr = self.start() as *mut u8;
        let buf_size = self.size() as usize;
        std::slice::from_raw_parts_mut(buf_ptr, buf_size)
    }
}

impl fmt::Debug for VMRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VMRange {{ start: 0x{:x?}, end: 0x{:x?}, size: 0x{:x?} }}",
            self.start,
            self.end,
            self.size()
        )
    }
}
