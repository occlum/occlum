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

/// Process scheduling nice value.
///
/// Lower values give a process a higher scheduling priority.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct NiceValue {
    value: i8,
}

impl NiceValue {
    pub const MAX: Self = Self { value: 19 };

    pub const MIN: Self = Self { value: -20 };

    /// Create a nice value from a raw value.
    ///
    /// The raw value given beyond the range are automatically adjusted
    /// to the nearest boundary value.
    pub fn new(raw: i8) -> Self {
        Self {
            value: raw.clamp(Self::MIN.value, Self::MAX.value),
        }
    }

    /// Convert to the raw value with range [19, -20].
    pub fn to_raw_val(self) -> i8 {
        self.value
    }
}

impl From<i32> for NiceValue {
    fn from(raw: i32) -> Self {
        let adj_raw = raw.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
        Self::new(adj_raw)
    }
}
