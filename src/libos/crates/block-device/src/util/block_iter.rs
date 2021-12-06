
/// An iterator of a set of blocks that covers a range of byte addresses.
///
/// 
pub struct RangeIter {
    cursor: usize,
    begin: usize,
    end: usize,
}

impl BlockIter {
    pub fn new(begin: usize, end: usize) -> Self {
        assert!(begin <= end);
        Self {
            cursor: begin,
            begin,
            end, 
        }
    }
}

pub type Range = core::ops::RangeTo<usize>;

impl Iterator for BlockIter {
    type Item = (BlockId, Range);

    fn next(&mut self) -> Option<Self::Item> {

    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}