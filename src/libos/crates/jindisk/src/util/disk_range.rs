//! Disk range.
use crate::prelude::*;

use std::cmp::PartialOrd;
use std::ops::Range;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DiskRange<T>(Range<T>);

impl<T: Copy + PartialOrd> DiskRange<T> {
    pub const fn new(range: Range<T>) -> Self {
        Self(range)
    }

    pub fn start(&self) -> T {
        self.0.start
    }

    pub fn end(&self) -> T {
        self.0.end
    }

    pub fn is_within_range(&self, target: T) -> bool {
        self.0.contains(&target)
    }

    pub fn is_sub_range(&self, rhs_range: &Self) -> bool {
        self.start() <= rhs_range.start() && self.end() >= rhs_range.end()
    }

    pub fn is_overlapped(&self, rhs_range: &Self) -> bool {
        !(self.end() <= rhs_range.start() || self.start() >= rhs_range.end())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub type LbaRange = DiskRange<Lba>;
pub type HbaRange = DiskRange<Hba>;

impl LbaRange {
    pub fn num_covered_blocks(&self) -> usize {
        (self.0.end - self.0.start.to_raw()).to_raw() as _
    }
}

/// Iterator for disk range.
pub struct DiskRangeIter {
    pub start: Lba,
    pub end: Lba,
}

impl DiskRangeIter {
    pub fn new(disk_range: &LbaRange) -> Self {
        Self {
            start: disk_range.start(),
            end: disk_range.end(),
        }
    }
}

impl Iterator for DiskRangeIter {
    type Item = Lba;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        // Exit condition
        if self.start >= self.end {
            return None;
        }

        let cur = self.start;
        self.start = self.start + 1 as _;
        Some(cur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_range() {
        let lba_range = LbaRange::new(Lba::new(25)..Lba::new(125));

        assert_eq!(lba_range.is_within_range(Lba::new(27)), true);
        assert_eq!(lba_range.is_within_range(Lba::new(23)), false);

        assert_eq!(lba_range.num_covered_blocks(), 100);

        let rhs_lba_range = LbaRange::new(Lba::new(10)..Lba::new(15));
        assert_eq!(lba_range.is_overlapped(&rhs_lba_range), false);

        let rhs_lba_range = LbaRange::new(Lba::new(10)..Lba::new(100));
        assert_eq!(lba_range.is_overlapped(&rhs_lba_range), true);

        let mut offset = lba_range.start();
        let lba_range_iter = DiskRangeIter::new(&lba_range);
        for lba in lba_range_iter {
            assert_eq!(lba, offset);
            offset = offset + 1 as _;
        }
        assert_eq!(lba_range.end(), offset);

        let sub_range = LbaRange::new(Lba::new(55)..Lba::new(75));
        assert_eq!(lba_range.is_sub_range(&sub_range), true);
    }
}
