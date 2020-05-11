use super::c_types::stack_t;
use crate::prelude::*;

pub const MINSIGSTKSZ: usize = 2048;

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum SigStackFlags {
    /// The alternate signal stack is enabled and the process is not executing on it
    EMPTY = 0,
    /// The process is currently executing on the alternate signal stack
    SS_ONSTACK = 1,
    /// The alternate signal stack is currently disabled
    SS_DISABLE = 2,
    /// The alternate signal stack has been marked to be autodisarmed
    SS_AUTODISARM = 1 << 31,
}

impl SigStackFlags {
    pub fn from_u32(bits: u32) -> Result<Self> {
        if bits > Self::SS_DISABLE as u32 && bits != Self::SS_AUTODISARM as u32 {
            return_errno!(EINVAL, "invalid bits for sig stack flags");
        }
        Ok(unsafe { core::mem::transmute(bits as u32) })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SigStack {
    sp: usize,
    flags: SigStackFlags,
    size: usize,
}

impl SigStack {
    pub fn from_c(ss_c: &stack_t) -> Result<Self> {
        Ok(Self {
            sp: ss_c.ss_sp as usize,
            flags: SigStackFlags::from_u32(ss_c.ss_flags as u32)?,
            size: ss_c.ss_size as usize,
        })
    }

    pub fn to_c(&self) -> stack_t {
        stack_t {
            ss_sp: self.sp as *mut c_void,
            ss_flags: self.flags as i32,
            ss_size: self.size,
        }
    }

    pub fn sp(&self) -> usize {
        self.sp
    }

    pub fn flags(&self) -> SigStackFlags {
        self.flags
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn update(&mut self, sp: usize, flags: SigStackFlags, size: usize) {
        self.sp = sp;
        self.flags = flags;
        self.size = size;
    }

    pub fn contains(&self, addr: usize) -> bool {
        addr >= self.sp && addr - self.sp < self.size
    }
}

impl Default for SigStack {
    fn default() -> Self {
        Self {
            sp: 0,
            flags: SigStackFlags::SS_DISABLE,
            size: 0,
        }
    }
}
