use crate::prelude::*;
use core::convert::TryFrom;

#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
#[repr(u8)]
pub enum PrioWhich {
    PRIO_PROCESS = 0,
    PRIO_PGRP = 1,
    PRIO_USER = 2,
}

impl TryFrom<i32> for PrioWhich {
    type Error = crate::error::Error;

    fn try_from(raw: i32) -> Result<Self> {
        if raw > Self::PRIO_USER as i32 || raw < Self::PRIO_PROCESS as i32 {
            return_errno!(EINVAL, "invalid which value");
        }
        Ok(unsafe { core::mem::transmute(raw as u8) })
    }
}

/// Process priority value
///
/// Lower values give a process a higher scheduling priority.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct NiceValue {
    value: i32,
}

impl NiceValue {
    const MAX_PRIO: i32 = 19;

    const MIN_PRIO: i32 = -20;

    pub fn max_value() -> Self {
        Self {
            value: Self::MAX_PRIO,
        }
    }

    pub fn min_value() -> Self {
        Self {
            value: Self::MIN_PRIO,
        }
    }

    pub fn raw_val(&self) -> i32 {
        self.value
    }

    /// Convert [19,-20] to priority value [39,0].
    pub fn to_priority_val(&self) -> i32 {
        self.value - Self::MIN_PRIO
    }

    /// Convert [19,-20] to rlimit style value [1,40].
    pub fn to_rlimit_val(&self) -> i32 {
        Self::MAX_PRIO - self.value + 1
    }
}

impl From<i32> for NiceValue {
    fn from(raw: i32) -> Self {
        let value = if raw < Self::MIN_PRIO {
            Self::MIN_PRIO
        } else if raw > Self::MAX_PRIO {
            Self::MAX_PRIO
        } else {
            raw
        };
        Self { value }
    }
}
