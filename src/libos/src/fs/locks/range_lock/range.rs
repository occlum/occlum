use super::*;

pub const OFFSET_MAX: usize = off_t::MAX as usize;

#[derive(Debug, Copy, Clone)]
pub struct FileRange {
    start: usize,
    end: usize,
}

impl FileRange {
    /// Create the range through C flock and opened file reference
    pub fn from_c_flock_and_file(lock: &c_flock, file: &FileRef) -> Result<Self> {
        let start = {
            let whence = RangeLockWhence::from_u16(lock.l_whence)?;
            match whence {
                RangeLockWhence::SEEK_SET => lock.l_start,
                RangeLockWhence::SEEK_CUR => file
                    .position()?
                    .checked_add(lock.l_start)
                    .ok_or_else(|| errno!(EOVERFLOW, "start overflow"))?,
                RangeLockWhence::SEEK_END => (file.metadata()?.size as off_t)
                    .checked_add(lock.l_start)
                    .ok_or_else(|| errno!(EOVERFLOW, "start overflow"))?,
            }
        };
        if start < 0 {
            return_errno!(EINVAL, "invalid start");
        }

        let (start, end) = if lock.l_len > 0 {
            let end = start
                .checked_add(lock.l_len)
                .ok_or_else(|| errno!(EOVERFLOW, "end overflow"))?;
            (start as usize, end as usize)
        } else if lock.l_len == 0 {
            (start as usize, OFFSET_MAX)
        } else {
            // len < 0, must recalculate the start
            let end = start;
            let new_start = start + lock.l_len;
            if new_start < 0 {
                return_errno!(EINVAL, "invalid len");
            }
            (new_start as usize, end as usize)
        };

        Ok(Self { start, end })
    }

    pub fn new(start: usize, end: usize) -> Result<Self> {
        if start >= end {
            return_errno!(EINVAL, "invalid parameters");
        }
        Ok(Self { start, end })
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn set_start(&mut self, new_start: usize) -> Result<FileRangeChange> {
        if new_start >= self.end {
            return_errno!(EINVAL, "invalid new start");
        }
        let old_start = self.start;
        self.start = new_start;
        let change = if new_start > old_start {
            FileRangeChange::Shrinked
        } else if new_start < old_start {
            FileRangeChange::Expanded
        } else {
            FileRangeChange::Same
        };
        Ok(change)
    }

    pub fn set_end(&mut self, new_end: usize) -> Result<FileRangeChange> {
        if new_end <= self.start {
            return_errno!(EINVAL, "invalid new end");
        }
        let old_end = self.end;
        self.end = new_end;
        let change = if new_end < old_end {
            FileRangeChange::Shrinked
        } else if new_end > old_end {
            FileRangeChange::Expanded
        } else {
            FileRangeChange::Same
        };
        Ok(change)
    }

    pub fn overlap_with(&self, other: &Self) -> Option<OverlapWith> {
        if self.start >= other.end || self.end <= other.start {
            return None;
        }

        let overlap = if self.start <= other.start && self.end < other.end {
            OverlapWith::ToLeft
        } else if self.start > other.start && self.end < other.end {
            OverlapWith::InMiddle
        } else if self.start > other.start && self.end >= other.end {
            OverlapWith::ToRight
        } else {
            OverlapWith::Includes
        };
        Some(overlap)
    }

    pub fn merge(&mut self, other: &Self) -> Result<FileRangeChange> {
        if self.end < other.start || other.end < self.start {
            return_errno!(EINVAL, "can not merge separated ranges");
        }

        let mut change = FileRangeChange::Same;
        if other.start < self.start {
            self.start = other.start;
            change = FileRangeChange::Expanded;
        }
        if other.end > self.end {
            self.end = other.end;
            change = FileRangeChange::Expanded;
        }
        Ok(change)
    }
}

#[derive(Debug)]
pub enum FileRangeChange {
    Same,
    Expanded,
    Shrinked,
}

/// The position of a range (say A) relative another overlapping range (say B).
#[derive(Debug)]
pub enum OverlapWith {
    /// The position where range A is to the left of B (A.start <= B.start && A.end < B.end).
    ToLeft,
    /// The position where range A is to the right of B (A.start > B.start && A.end >= B.end).
    ToRight,
    /// The position where range A is in the middle of B (A.start > B.start && A.end < B.end).
    InMiddle,
    /// The position where range A includes B (A.start <= B.start && A.end >= B.end).
    Includes,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
#[repr(u16)]
pub enum RangeLockWhence {
    SEEK_SET = 0,
    SEEK_CUR = 1,
    SEEK_END = 2,
}

impl RangeLockWhence {
    pub fn from_u16(whence: u16) -> Result<Self> {
        Ok(match whence {
            0 => RangeLockWhence::SEEK_SET,
            1 => RangeLockWhence::SEEK_CUR,
            2 => RangeLockWhence::SEEK_END,
            _ => return_errno!(EINVAL, "Invalid whence"),
        })
    }
}
