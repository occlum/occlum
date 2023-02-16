//! Range query context.
use crate::prelude::*;
use bitvec::macros::internal::funty::Fundamental;

/// Range query context.
#[derive(Debug)]
pub struct RangeQueryCtx {
    lba_range: LbaRange,
    range_ctx: BitMap,
}

impl RangeQueryCtx {
    pub fn build_from(offset: usize, buf: &'a mut [u8]) -> Self {
        let (start, end) = (
            Lba::from_byte_offset_aligned(offset).unwrap(),
            Lba::from_byte_offset_aligned(offset + buf.len()).unwrap(),
        );
        let lba_range = LbaRange::new(start..end);
        let range_ctx = BitMap::repeat(false, lba_range.num_covered_blocks());

        Self {
            lba_range,
            range_ctx,
        }
    }

    pub fn target_range(&self) -> &LbaRange {
        &self.lba_range
    }

    pub fn num_queried_blocks(&self) -> usize {
        self.lba_range.num_covered_blocks()
    }

    pub fn idx(&self, lba: Lba) -> usize {
        (lba - self.lba_range.start().to_raw()).to_raw() as _
    }

    pub fn is_completed(&self) -> bool {
        for completed in self.range_ctx.iter() {
            if !completed {
                return false;
            }
        }
        true
    }

    pub fn collect_uncompleted(&self) -> Vec<(usize, Lba)> {
        self.range_ctx
            .iter()
            .enumerate()
            .filter(|(_, completed)| completed.as_bool())
            .map(|(idx, _)| (idx, self.lba_range.start() + idx as _))
            .collect()
    }

    pub fn complete(&mut self, lba: Lba) {
        let idx = self.idx(lba);
        self.range_ctx.set(idx, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_ctx() {
        const NUM: usize = 4;
        let mut ctx = RangeQueryCtx::build_from(BLOCK_SIZE, &mut [0u8; NUM * BLOCK_SIZE]);

        assert_eq!(ctx.num_queried_blocks(), NUM);
        assert_eq!(*ctx.target_range(), LbaRange::new(Lba::new(1)..Lba::new(5)));

        ctx.complete(Lba::new(2));
        ctx.complete(Lba::new(4));

        assert_eq!(
            ctx.collect_uncompleted(),
            vec![(0, Lba::new(1)), (2, Lba::new(3))]
        );

        ctx.complete(Lba::new(1));
        ctx.complete(Lba::new(3));

        assert_eq!(ctx.is_completed(), true);
    }
}
