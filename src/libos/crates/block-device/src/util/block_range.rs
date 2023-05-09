//! BlockRangeIter is an iterator for blocks within a specific range.
//! BlockRange describes sub-range information about each block.
use crate::prelude::*;

/// Given a range and iterate sub-range for each block.
#[derive(Clone, Debug)]
pub struct BlockRangeIter {
    pub begin: usize,
    pub end: usize,
    pub block_size: usize,
}

/// Describe the range for one block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockRange {
    pub block_id: Bid,
    pub begin: usize,
    pub end: usize,
    pub block_size: usize,
}

impl Iterator for BlockRangeIter {
    type Item = BlockRange;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        // Exit condition
        if self.begin >= self.end {
            return None;
        }

        // Construct sub-range of next block
        let sub_range = {
            let block_id = Bid::from_byte_offset(self.begin);
            let begin = self.begin % self.block_size;
            let end = if block_id == Bid::from_byte_offset(self.end) {
                self.end % self.block_size
            } else {
                self.block_size
            };
            let block_size = self.block_size;
            BlockRange {
                block_id,
                begin,
                end,
                block_size,
            }
        };

        self.begin += sub_range.len();
        Some(sub_range)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact_size = {
            let begin_bid = Bid::from_byte_offset(self.begin);
            let end_bid = Bid::from_byte_offset(self.end);
            if self.end % self.block_size == 0 {
                (end_bid - begin_bid.to_raw()).to_raw() as _
            } else {
                (end_bid - begin_bid.to_raw()).to_raw() as usize + 1
            }
        };
        (exact_size, Some(exact_size))
    }
}

impl ExactSizeIterator for BlockRangeIter {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        debug_assert!(upper == Some(lower));
        lower
    }
}

impl BlockRange {
    /// Return block length.
    pub fn len(&self) -> usize {
        self.end - self.begin
    }

    /// Whether the range covers a whole block.
    pub fn is_full(&self) -> bool {
        self.len() == self.block_size
    }

    /// Whether the range is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Describe the begin offset of this block from the whole range.
    pub fn origin_begin(&self) -> usize {
        self.block_id.to_raw() as usize * self.block_size + self.begin
    }

    /// Describe the end offset of this block from the whole range.
    pub fn origin_end(&self) -> usize {
        self.block_id.to_raw() as usize * self.block_size + self.end
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn block_range_iter() {
        let mut iter = BlockRangeIter {
            begin: 0x125,
            end: 0x2025,
            block_size: BLOCK_SIZE,
        };
        assert_eq!(iter.clone().count(), iter.len());

        assert_eq!(
            iter.next(),
            Some(BlockRange {
                block_id: Bid::new(0),
                begin: 0x125,
                end: 0x1000,
                block_size: BLOCK_SIZE,
            })
        );

        assert_eq!(
            iter.next(),
            Some(BlockRange {
                block_id: Bid::new(1),
                begin: 0,
                end: 0x1000,
                block_size: BLOCK_SIZE,
            })
        );

        assert_eq!(
            iter.next(),
            Some(BlockRange {
                block_id: Bid::new(2),
                begin: 0,
                end: 0x25,
                block_size: BLOCK_SIZE,
            })
        );

        assert_eq!(iter.next(), None);
    }
}
