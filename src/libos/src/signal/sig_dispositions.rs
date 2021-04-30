use std::fmt;

use super::constants::*;
use super::{SigAction, SigNum};
use crate::prelude::*;

#[derive(Copy, Clone)]
pub struct SigDispositions {
    // SigNum -> SigAction
    map: [SigAction; COUNT_ALL_SIGS],
}

impl SigDispositions {
    pub fn new() -> Self {
        Self {
            map: [Default::default(); COUNT_ALL_SIGS],
        }
    }

    pub fn get(&self, num: SigNum) -> SigAction {
        let idx = Self::num_to_idx(num);
        self.map[idx]
    }

    pub fn set(&mut self, num: SigNum, sa: SigAction) {
        let idx = Self::num_to_idx(num);
        self.map[idx] = sa;
    }

    pub fn set_default(&mut self, num: SigNum) {
        let idx = Self::num_to_idx(num);
        self.map[idx] = SigAction::Dfl;
    }

    pub fn iter<'a>(&'a self) -> SigDispositionsIter<'a> {
        SigDispositionsIter::new(self)
    }

    // inherit sigdispositions for child process, user defined sigaction should be set to default
    pub fn inherit(&mut self) {
        for mut sigaction in &mut self.map {
            match sigaction {
                SigAction::User {
                    handler_addr,
                    flags,
                    restorer_addr,
                    mask,
                } => {
                    *sigaction = SigAction::Dfl;
                }
                _ => {}
            }
        }
    }

    fn num_to_idx(num: SigNum) -> usize {
        (num.as_u8() - MIN_STD_SIG_NUM) as usize
    }

    fn idx_to_num(idx: usize) -> SigNum {
        unsafe { SigNum::from_u8_unchecked(idx as u8 + MIN_STD_SIG_NUM) }
    }
}

pub struct SigDispositionsIter<'a> {
    next_idx: usize,
    dispos: &'a SigDispositions,
}

impl<'a> SigDispositionsIter<'a> {
    pub fn new(dispos: &'a SigDispositions) -> Self {
        SigDispositionsIter {
            next_idx: 0,
            dispos: dispos,
        }
    }
}

impl<'a> std::iter::Iterator for SigDispositionsIter<'a> {
    type Item = (SigNum, &'a SigAction);

    fn next(&mut self) -> Option<Self::Item> {
        let map = &self.dispos.map;
        if self.next_idx >= map.len() {
            return None;
        }

        let item = {
            let signum = SigDispositions::idx_to_num(self.next_idx);
            let action = &map[self.next_idx];
            Some((signum, action))
        };
        self.next_idx += 1;
        item
    }
}

impl Default for SigDispositions {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SigDispositions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SigDispositions (only none-default is shown) ");
        let non_default_dispositions = self.iter().filter(|(_, action)| **action != SigAction::Dfl);
        f.debug_map().entries(non_default_dispositions).finish()
    }
}
