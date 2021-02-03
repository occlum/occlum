use core::arch::x86_64::{_fxrstor, _fxsave};
use std::mem::MaybeUninit;

use aligned::{Aligned, A16};

use crate::prelude::*;

/// Floating point registers
///
/// Note. The area is used to save fxsave result
//#[derive(Clone, Copy)]
#[repr(C)]
pub struct FpRegs {
    inner: Aligned<A16, [u8; 512]>,
}

impl FpRegs {
    /// Save the current CPU floating pointer states to an instance of FpRegs
    pub fn save() -> Self {
        let mut fpregs = MaybeUninit::<Self>::uninit();
        unsafe {
            _fxsave(fpregs.as_mut_ptr() as *mut u8);
            fpregs.assume_init()
        }
    }

    /// Restore the current CPU floating pointer states from this FpRegs instance
    pub fn restore(&self) {
        unsafe { _fxrstor(self.inner.as_ptr()) };
    }

    /// Construct a FpRegs from a slice of u8.
    ///
    /// It is up to the caller to ensure that the src slice contains data that
    /// is the xsave/xrstor format.
    pub unsafe fn from_slice(src: &[u8]) -> Self {
        let mut uninit = MaybeUninit::<Self>::uninit();
        let dst_buf: &mut [u8] = std::slice::from_raw_parts_mut(
            uninit.as_mut_ptr() as *mut u8,
            std::mem::size_of::<FpRegs>(),
        );
        dst_buf.copy_from_slice(&src);
        uninit.assume_init()
    }

    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_ref()
    }
}
