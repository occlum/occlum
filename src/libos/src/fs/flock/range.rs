use super::*;

pub const RANGE_EOF: usize = usize::max_value();

#[derive(Debug, Copy, Clone)]
pub struct FlockRange {
    start: usize,
    end: usize,
}

impl FlockRange {
    /// Create the lock range through C flock and file reference
    pub fn from_c_flock_and_file(lock: &c_flock, file: &FileRef) -> Result<Self> {
        let start = {
            let whence = FlockWhence::from_u16(lock.l_whence)?;
            match whence {
                FlockWhence::SEEK_SET => lock.l_start,
                FlockWhence::SEEK_CUR => file
                    .position()?
                    .checked_add(lock.l_start)
                    .ok_or_else(|| errno!(EOVERFLOW, "start overflow"))?,
                FlockWhence::SEEK_END => (file.metadata()?.size as off_t)
                    .checked_add(lock.l_start)
                    .ok_or_else(|| errno!(EOVERFLOW, "start overflow"))?,
            }
        };
        FlockRange::new(start, lock.l_len)
    }

    /// Create the lock range through the start offset and length
    /// length is 0 means until EOF of the file
    pub fn new(start: off_t, len: off_t) -> Result<Self> {
        if start < 0 {
            return_errno!(EINVAL, "invalid start");
        }
        let (start, end) = if len > 0 {
            let end = start
                .checked_add(len - 1)
                .ok_or_else(|| errno!(EOVERFLOW, "end overflow"))?;
            (start as usize, end as usize)
        } else if len == 0 {
            (start as usize, RANGE_EOF)
        } else {
            // len < 0, must recalculate the start
            let end = start - 1;
            let new_start = start + len;
            if new_start < 0 {
                return_errno!(EINVAL, "invalid len");
            }
            (new_start as usize, end as usize)
        };
        Ok(Self { start, end })
    }

    pub fn len(&self) -> usize {
        self.end - self.start + 1
    }

    pub fn start(&self) -> usize {
        self.start
    }

    /// Return the `FlockRangeReport` if success
    pub fn set_start(&mut self, new_start: usize) -> Result<FlockRangeReport> {
        if new_start > self.end {
            return_errno!(EINVAL, "invalid new start");
        }
        let old_start = self.start;
        self.start = new_start;
        let report = if new_start > old_start {
            FlockRangeReport::Shrink
        } else if new_start < old_start {
            FlockRangeReport::Expand
        } else {
            FlockRangeReport::Constant
        };
        Ok(report)
    }

    pub fn end(&self) -> usize {
        self.end
    }

    /// Return the `FlockRangeReport` if success
    pub fn set_end(&mut self, new_end: usize) -> Result<FlockRangeReport> {
        if new_end < self.start {
            return_errno!(EINVAL, "invalid new end");
        }
        let old_end = self.end;
        self.end = new_end;
        let report = if new_end < old_end {
            FlockRangeReport::Shrink
        } else if new_end > old_end {
            FlockRangeReport::Expand
        } else {
            FlockRangeReport::Constant
        };
        Ok(report)
    }

    pub fn overlap_with(&self, other: &Self) -> bool {
        self.start <= other.end && self.end >= other.start
    }

    pub fn left_overlap_with(&self, other: &Self) -> bool {
        if !self.overlap_with(other) {
            return false;
        }
        self.start <= other.start && self.end < other.end
    }

    pub fn middle_overlap_with(&self, other: &Self) -> bool {
        if !self.overlap_with(other) {
            return false;
        }
        self.start > other.start && self.end < other.end
    }

    pub fn right_overlap_with(&self, other: &Self) -> bool {
        if !self.overlap_with(other) {
            return false;
        }
        self.start > other.start && self.end >= other.end
    }

    pub fn adjacent_or_overlap_with(&self, other: &Self) -> bool {
        let adjacent = self.end == other.start - 1 || other.end == self.start - 1;
        adjacent || self.overlap_with(other)
    }

    pub fn in_front_of(&self, other: &Self) -> bool {
        self.end < other.start - 1
    }

    pub fn in_front_of_or_adjacent_before(&self, other: &Self) -> bool {
        self.end < other.start
    }

    /// Return the `FlockRangeReport` if success
    pub fn merge(&mut self, other: &Self) -> Result<FlockRangeReport> {
        if !self.adjacent_or_overlap_with(other) {
            return_errno!(EINVAL, "can not merge");
        }
        let mut report = FlockRangeReport::Constant;
        if other.start < self.start {
            self.start = other.start;
            report = FlockRangeReport::Expand;
        }
        if other.end > self.end {
            self.end = other.end;
            report = FlockRangeReport::Expand;
        }
        Ok(report)
    }
}

pub enum FlockRangeReport {
    Constant,
    Shrink,
    Expand,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
#[repr(u16)]
pub enum FlockWhence {
    SEEK_SET = 0,
    SEEK_CUR = 1,
    SEEK_END = 2,
}

impl FlockWhence {
    pub fn from_u16(whence: u16) -> Result<Self> {
        Ok(match whence {
            0 => FlockWhence::SEEK_SET,
            1 => FlockWhence::SEEK_CUR,
            2 => FlockWhence::SEEK_END,
            _ => return_errno!(EINVAL, "Invalid whence"),
        })
    }
}
