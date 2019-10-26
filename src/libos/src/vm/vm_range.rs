use super::*;

#[derive(Clone, Copy, Default, PartialEq)]
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
            (false, false) => Vec::new(),
            (false, true) => unsafe {
                vec![VMRange::from_unchecked(self_start.max(other_end), self_end)]
            },
            (true, false) => unsafe {
                vec![VMRange::from_unchecked(
                    self_start,
                    self_end.min(other_start),
                )]
            },
            (true, true) => unsafe {
                vec![
                    VMRange::from_unchecked(self_start, other_start),
                    VMRange::from_unchecked(other_end, self_end),
                ]
            },
        }
    }

    pub fn intersect(&self, other: &VMRange) -> Option<VMRange> {
        let intersection_start = self.start().max(other.start());
        let intersection_end = self.end().min(other.end());
        if intersection_start > intersection_end {
            return None;
        }
        unsafe {
            Some(VMRange::from_unchecked(
                intersection_start,
                intersection_end,
            ))
        }
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
