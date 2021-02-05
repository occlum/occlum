use std::ops::RangeInclusive;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
#[cfg(not(feature = "sgx"))]
use std::sync::{Mutex, MutexGuard};
#[cfg(feature = "sgx")]
use std::sync::{SgxMutex as Mutex, SgxMutexGuard as MutexGuard};

use crate::page_cache::Page;

/// A few tuning knobs for the sequential read tracker.
pub const MIN_PREFETCH_SIZE: usize = Page::size();
pub const MAX_PREFETCH_SIZE: usize = 64 * Page::size();
pub const MAX_CONCURRENCY: usize = 3;

/// A tracker for multiple concurrent sequential reads on a file.
///
/// If the tracker decides that a read is sequential, then it can help further decide
/// how much data to prefetch.
pub struct SeqRdTracker {
    trackers: [Mutex<Tracker>; MAX_CONCURRENCY],
}

// An internal tracker for a single thread of sequential reads.
struct Tracker {
    seq_window: RangeInclusive<usize>,
    prefetch_size: usize,
}

/// A sequential read.
pub struct SeqRd<'a> {
    tracker: MutexGuard<'a, Tracker>,
    offset: usize,
    len: usize,
}

// Implementation for SeqRdTracker

impl SeqRdTracker {
    pub fn new() -> Self {
        let trackers = array_init::array_init(|_| Mutex::new(Tracker::new()));
        Self { trackers }
    }

    /// Accept a new read.
    ///
    /// By accepting a new read, we track the read and guess---according to the
    /// previously accepted reads---whether the new read is sequential. If so,
    /// we return an object that represents the sequential read, which can in turn
    /// give a "good" suggestion for the amount of data to prefetch.
    pub fn accept(&self, offset: usize, len: usize) -> Option<SeqRd<'_>> {
        // Try to find a tracker of sequential reads that matches the new read.
        //
        // If not found, we pick a "victim" tracker to track the potentially new
        // thread of sequential reads starting from this read.
        let mut victim_tracker_opt: Option<MutexGuard<'_, Tracker>> = None;
        for (tracker_i, tracker_lock) in self.trackers.iter().enumerate() {
            let tracker = match tracker_lock.try_lock().ok() {
                Some(tracker) => tracker,
                None => continue,
            };

            if tracker.check_sequential(offset) {
                return Some(SeqRd::new(tracker, offset, len));
            } else {
                // Victim selection: we prefer the tracker with greater prefetch size.
                if let Some(victim_tracker) = victim_tracker_opt.as_mut() {
                    if victim_tracker.prefetch_size < tracker.prefetch_size {
                        victim_tracker_opt = Some(tracker);
                    }
                } else {
                    victim_tracker_opt = Some(tracker);
                }
            }
        }

        let mut victim_tracker = victim_tracker_opt?;
        victim_tracker.restart_from(offset, len);
        None
    }
}

// Implementation for Tracker

// The value of the prefetch size of a tracker that has not been able to track any
// sequential reads. This value is chosen so that our criterion of replacing a tracker
// can be simplified to "always choose the one with the greatest prefetch size".
const INVALID_PREFETCH_SIZE: usize = usize::max_value();

impl Tracker {
    pub fn new() -> Self {
        Self {
            seq_window: (0..=0),
            prefetch_size: INVALID_PREFETCH_SIZE,
        }
    }

    pub fn check_sequential(&self, offset: usize) -> bool {
        self.seq_window.contains(&offset)
    }

    pub fn restart_from(&mut self, offset: usize, len: usize) {
        self.seq_window = (offset + len / 2)..=(offset + len);
        self.prefetch_size = INVALID_PREFETCH_SIZE;
    }
}

// Implementation for SeqRd

impl<'a> SeqRd<'a> {
    fn new(mut tracker: MutexGuard<'a, Tracker>, offset: usize, len: usize) -> Self {
        if tracker.prefetch_size == INVALID_PREFETCH_SIZE {
            tracker.prefetch_size = MIN_PREFETCH_SIZE;
        }
        Self {
            tracker,
            offset,
            len,
        }
    }

    pub fn prefetch_size(&self) -> usize {
        self.tracker.prefetch_size
    }

    pub fn complete(mut self, read_bytes: usize) {
        debug_assert!(read_bytes > 0);
        self.tracker.seq_window = {
            let low = self.offset + read_bytes / 2;
            let upper = self.offset + read_bytes;
            low..=upper
        };

        self.tracker.prefetch_size *= 2;
        let max_prefetch_size = MAX_PREFETCH_SIZE.min(self.len * 4);
        if self.tracker.prefetch_size > max_prefetch_size {
            self.tracker.prefetch_size = max_prefetch_size;
        }
    }
}

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
