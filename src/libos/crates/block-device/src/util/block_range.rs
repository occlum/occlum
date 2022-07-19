//! BlockRangeIter is an iterator for blocks within a specific range.
//! BlockRange describes sub-range information about each block.
use crate::prelude::*;

/// Given a range and iterate sub-range for each block.
#[derive(Debug)]
pub struct BlockRangeIter {
    pub begin: usize,
    pub end: usize,
    pub block_size: usize,
}

/// Describe the range for one block.
#[derive(Debug, Eq, PartialEq)]
pub struct BlockRange {
    pub block_id: BlockId,
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
            let block_id = self.begin / self.block_size;
            let begin = self.begin % self.block_size;
            let end = if block_id == self.end / self.block_size {
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
        self.block_id * self.block_size + self.begin
    }

    /// Describe the end offset of this block from the whole range.
    pub fn origin_end(&self) -> usize {
        self.block_id * self.block_size + self.end
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

        assert_eq!(
            iter.next(),
            Some(BlockRange {
                block_id: 0,
                begin: 0x125,
                end: 0x1000,
                block_size: BLOCK_SIZE,
            })
        );

        assert_eq!(
            iter.next(),
            Some(BlockRange {
                block_id: 1,
                begin: 0,
                end: 0x1000,
                block_size: BLOCK_SIZE,
            })
        );

        assert_eq!(
            iter.next(),
            Some(BlockRange {
                block_id: 2,
                begin: 0,
                end: 0x25,
                block_size: BLOCK_SIZE,
            })
        );

        assert_eq!(iter.next(), None);
    }
}
