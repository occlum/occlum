use std::fmt;
use std::iter;
use std::ops::{Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, Not, Sub, SubAssign};

use super::constants::MIN_STD_SIG_NUM;
use super::{sigset_t, SigNum};
use crate::events::EventFilter;
use crate::prelude::*;

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct SigSet {
    bits: u64,
}

impl SigSet {
    pub const fn new_empty() -> Self {
        Self::from_c(0 as sigset_t)
    }

    pub const fn new_full() -> Self {
        Self::from_c(!0 as sigset_t)
    }

    pub const fn from_c(bits: sigset_t) -> Self {
        let bits = bits as u64;
        SigSet { bits }
    }

    pub fn to_c(&self) -> sigset_t {
        self.bits as sigset_t
    }

    pub fn as_u64(&self) -> u64 {
        self.bits
    }

    pub fn empty(&self) -> bool {
        self.bits == 0
    }

    pub fn full(&self) -> bool {
        self.bits == !0
    }

    pub fn count(&self) -> usize {
        self.bits.count_ones() as usize
    }

    pub fn contains(&self, signum: SigNum) -> bool {
        let idx = Self::num_to_idx(signum);
        (self.bits & (1_u64 << idx)) != 0
    }

    pub fn iter(&self) -> SigSetIter {
        SigSetIter::new(self)
    }

    fn num_to_idx(num: SigNum) -> usize {
        (num.as_u8() - MIN_STD_SIG_NUM) as usize
    }

    fn idx_to_num(idx: usize) -> SigNum {
        debug_assert!(idx < 64);
        unsafe { SigNum::from_u8_unchecked((idx + 1) as u8) }
    }
}

pub struct SigSetIter<'a> {
    sigset: &'a SigSet,
    next_idx: usize,
}

impl<'a> SigSetIter<'a> {
    pub fn new(sigset: &'a SigSet) -> Self {
        let next_idx = 0;
        Self { sigset, next_idx }
    }
}

impl<'a> iter::Iterator for SigSetIter<'a> {
    type Item = SigNum;

    fn next(&mut self) -> Option<Self::Item> {
        let bits = &self.sigset.bits;
        while self.next_idx < 64 && (*bits & (1 << self.next_idx)) == 0 {
            self.next_idx += 1;
        }
        if self.next_idx == 64 {
            return None;
        }
        let item = SigSet::idx_to_num(self.next_idx);
        self.next_idx += 1;
        Some(item)
    }
}

impl From<SigNum> for SigSet {
    fn from(signum: SigNum) -> SigSet {
        let mut sigset = SigSet::new_empty();
        sigset += signum;
        sigset
    }
}

impl Not for SigSet {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_c(!self.bits)
    }
}

impl BitOr for SigSet {
    type Output = Self;

    fn bitor(mut self, rhs: Self) -> Self {
        self |= rhs;
        self
    }
}

impl BitOrAssign for SigSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.bits |= rhs.bits;
    }
}

impl BitAnd for SigSet {
    type Output = Self;

    fn bitand(mut self, rhs: Self) -> Self {
        self &= rhs;
        self
    }
}

impl BitAndAssign for SigSet {
    fn bitand_assign(&mut self, rhs: Self) {
        self.bits &= rhs.bits;
    }
}

impl Add<SigNum> for SigSet {
    type Output = Self;

    fn add(mut self, rhs: SigNum) -> Self {
        self += rhs;
        self
    }
}

impl AddAssign<SigNum> for SigSet {
    fn add_assign(&mut self, rhs: SigNum) {
        let idx = Self::num_to_idx(rhs);
        self.bits |= 1_u64 << idx;
    }
}

impl Sub<SigNum> for SigSet {
    type Output = Self;

    fn sub(mut self, rhs: SigNum) -> Self {
        self -= rhs;
        self
    }
}

impl SubAssign<SigNum> for SigSet {
    fn sub_assign(&mut self, rhs: SigNum) {
        let idx = Self::num_to_idx(rhs);
        self.bits &= !(1_u64 << idx);
    }
}

impl fmt::Debug for SigSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SigSet {{ ");
        match self.count() {
            0..=32 => {
                f.debug_list().entries(self.iter()).finish();
            }
            33..=63 => {
                write!(f, "All except ");
                let except_sigset = !*self;
                f.debug_list().entries(except_sigset.iter()).finish();
            }
            64 => {
                write!(f, "None");
            }
            _ => unreachable!(),
        }
        write!(f, " }}")
    }
}

impl EventFilter<SigNum> for SigSet {
    fn filter(&self, event: &SigNum) -> bool {
        self.contains(*event)
    }
}
