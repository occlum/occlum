use super::super::c_types::*;
use super::super::constants::*;
use super::super::{SigNum, Signal};
use crate::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct KernelSignal {
    num: SigNum,
}

impl KernelSignal {
    pub fn new(num: SigNum) -> Self {
        Self { num }
    }
}

impl Signal for KernelSignal {
    fn num(&self) -> SigNum {
        self.num
    }

    fn to_info(&self) -> siginfo_t {
        let info = siginfo_t::new(self.num, SI_KERNEL);
        info
    }
}
