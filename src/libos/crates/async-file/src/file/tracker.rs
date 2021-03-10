#[cfg(feature = "sgx")]
use std::prelude::v1::*;

use crate::page_cache::Page;

/// A few tuning knobs for the sequential read tracker.
pub const MIN_PREFETCH_SIZE: usize = Page::size();
pub const MAX_PREFETCH_SIZE: usize = 64 * Page::size();

/// A read tracker that can determine whether a new read is sequential or not.
pub struct SeqRdTracker {
    // A new read is considered sequential if and only if the new read starts at
    // this position.
    last_rd_end: usize,
    // The end of the region that we have issued prefetch
    prefetch_end: usize,
    // The size of the next prefetch, increasing with the number of consecutive
    // sequential reads. If the size is 0, then the last read received by the
    // tracker is not sequential.
    prefetch_size: usize,
}

impl SeqRdTracker {
    pub fn new() -> Self {
        Self {
            last_rd_end: 0,
            prefetch_end: 0,
            prefetch_size: 0,
        }
    }

    pub fn track<'a>(&'a mut self, offset: usize, len: usize) -> NewRead<'a> {
        // Handle the cases when the new read is NOT considered sequential
        if offset != self.last_rd_end {
            self.prefetch_size = 0;
            return NewRead::Random {
                tracker: self,
                offset,
            };
        }

        // If this new read is the first sequential read
        if self.prefetch_size == 0 {
            self.prefetch_end = offset;
            self.prefetch_size = MIN_PREFETCH_SIZE;
        }
        NewRead::Sequential {
            tracker: self,
            offset,
            len,
        }
    }
}

pub enum NewRead<'a> {
    Sequential {
        tracker: &'a mut SeqRdTracker,
        offset: usize,
        len: usize,
    },
    Random {
        tracker: &'a mut SeqRdTracker,
        offset: usize,
    },
}

impl<'a> NewRead<'a> {
    pub fn prefetch_size(&self) -> usize {
        match self {
            Self::Sequential {
                tracker,
                offset,
                len,
            } => {
                // If the new read attempts to read the range beyond the end of the
                // prefetched region, we need to issue new prefetch
                if *offset + *len >= tracker.prefetch_end {
                    tracker.prefetch_size
                } else {
                    0
                }
            }
            Self::Random { .. } => 0,
        }
    }

    pub fn complete(self, read_nbytes: usize) {
        match self {
            Self::Sequential { tracker, len, .. } => {
                tracker.last_rd_end += read_nbytes;
                tracker.prefetch_end += len + tracker.prefetch_size;
                tracker.prefetch_size = MAX_PREFETCH_SIZE.min(tracker.prefetch_size * 2);
            }
            Self::Random { tracker, offset } => {
                tracker.last_rd_end = offset + read_nbytes;
            }
        }
    }
}

/*
#[cfg(test)]
mod test {
    use self::helper::SeqRds;
    use super::*;

    #[test]
    fn read_seq_one_thread() {
        let tracker = SeqRdTracker::new();

        let mut seq_rds = SeqRds::new(0, 4096, 10);
        seq_rds.for_each(|(offset, buf_size)| {
            let seq_rd = tracker.accept(offset, buf_size);
            assert!(seq_rd.is_some());
            let seq_rd = seq_rd.unwrap();
            //println!("prefetch size = {}", seq_rd.prefetch_size());
            seq_rd.complete(buf_size);
        });
    }

    #[test]
    fn read_seq_multi_threads() {
        let tracker = SeqRdTracker::new();

        let mixed_seq_rds = {
            let seq_rds0 = SeqRds::new(0, 4096, 10);
            let seq_rds1 = SeqRds::new(12345678, 1024, 8);
            seq_rds0.interleave(seq_rds1)
        };

        let mut num_non_seq_rds = 0;
        mixed_seq_rds.for_each(|(offset, buf_size)| {
            if let Some(seq_rd) = tracker.accept(offset, buf_size) {
                //println!("offset = {}, buf_size = {}: prefetch_size = {}", offset, buf_size, seq_rd.prefetch_size());
                seq_rd.complete(buf_size);
            } else {
                //println!("offset = {}, buf_size = {}: non-sequential", offset, buf_size);
                num_non_seq_rds += 1;
            }
        });
        assert!(num_non_seq_rds <= 1);
    }

    #[test]
    fn read_rand() {
        let tracker = SeqRdTracker::new();
        let random_rds = [(8, 16), (128, 16), (1024, 4)];
        random_rds.into_iter().for_each(|(offset, buf_size)| {
            let seq_rd = tracker.accept(*offset, *buf_size);
            assert!(seq_rd.is_none());
        });
    }

    mod helper {
        use super::*;

        pub struct SeqRds {
            offset: usize,
            buf_size: usize,
            nrepeats_remain: usize,
        }

        impl SeqRds {
            pub fn new(offset: usize, buf_size: usize, nrepeats: usize) -> Self {
                Self {
                    offset,
                    buf_size,
                    nrepeats_remain: nrepeats,
                }
            }
        }

        impl Iterator for SeqRds {
            type Item = (usize /* offset */, usize /* buf_size */);

            fn next(&mut self) -> Option<Self::Item> {
                if self.nrepeats_remain == 0 {
                    return None;
                }

                let new_rd = (self.offset, self.buf_size);
                self.offset += self.buf_size;
                self.nrepeats_remain -= 1;
                Some(new_rd)
            }
        }
    }
}
*/
