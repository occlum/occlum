use std::ops::{Deref, DerefMut};

use super::vm_perms::VMPerms;
use super::vm_range::VMRange;
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VMArea {
    range: VMRange,
    perms: VMPerms,
}

impl VMArea {
    pub fn new(range: VMRange, perms: VMPerms) -> Self {
        Self { range, perms }
    }

    pub fn perms(&self) -> VMPerms {
        self.perms
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn set_perms(&mut self, new_perms: VMPerms) {
        self.perms = new_perms;
    }

    pub fn subtract(&self, other: &VMRange) -> Vec<VMArea> {
        self.deref()
            .subtract(other)
            .iter()
            .map(|range| VMArea::new(*range, self.perms()))
            .collect()
    }
}

impl Deref for VMArea {
    type Target = VMRange;

    fn deref(&self) -> &Self::Target {
        &self.range
    }
}

impl DerefMut for VMArea {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.range
    }
}
