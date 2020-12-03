use bit_vec::BitVec;

use crate::executor::EXECUTOR;

/// The set of executor threads that a task can be scheduled to.
#[derive(Debug, Clone, PartialEq)]
pub struct Affinity {
    bits: BitVec<u32>,
}

impl Affinity {
    /// The max number of executor threads in a set.
    pub fn max_threads() -> usize {
        EXECUTOR.parallelism() as usize
    }

    /// A full set of executor threads.
    pub fn new_full() -> Self {
        let bits = BitVec::from_elem(Self::max_threads(), true);
        Self { bits }
    }

    /// A empty set of executor threads.
    pub fn new_empty() -> Self {
        let bits = BitVec::from_elem(Self::max_threads(), false);
        Self { bits }
    }

    /// Returns whether the set is full.
    pub fn is_full(&self) -> bool {
        self.bits.all()
    }

    /// Returns whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.bits.none()
    }

    /// Returns the number of threads in the set.
    pub fn count(&self) -> usize {
        self.bits.iter().filter(|x| *x).count()
    }

    /// Set whether the i-th thread is in the set.
    pub fn set(&mut self, i: usize, b: bool) {
        self.bits.set(i, b);
    }

    /// Get whether the i-th thread is in the set.
    pub fn get(&self, i: usize) -> bool {
        self.bits.get(i).unwrap()
    }

    /// Returns an iterator that allows accessing the underlying bits.
    pub fn iter(&self) -> impl Iterator<Item = bool> + '_ {
        self.bits.iter()
    }
}
