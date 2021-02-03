use core::arch::x86_64::{_fxrstor, _fxsave};
use std::mem::MaybeUninit;

use aligned::{Aligned, A16};

use crate::prelude::*;

/// The floating point state of CPU.
#[derive(Clone)]
#[repr(C)]
pub struct FpRegs {
    buf: Aligned<A16, [u8; 512]>,
    is_valid: bool,
}

impl FpRegs {
    /// Create a new instance.
    ///
    /// Note that a newly-created instance's floating point state is not
    /// initialized, thus considered invalid (i.e., `self.is_valid() == false`).
    pub fn new() -> Self {
        let buf = Aligned(unsafe { MaybeUninit::uninit().assume_init() });
        let is_valid = false;
        Self { buf, is_valid }
    }

    /// Save CPU's current floating pointer states into this instance.
    pub fn save(&mut self) {
        unsafe {
            _fxsave(self.buf.as_mut_ptr() as *mut u8);
        }
        self.is_valid = true;
    }

    /// Save the floating state given by a slice of u8.
    ///
    /// It is the caller's responsibility to ensure that the source slice contains
    /// data that is in xsave/xrstor format. The slice must have a length of 512 bytes.
    ///
    /// After calling this method, the state of the instance will be considered valid.
    pub unsafe fn save_from_slice(&mut self, src: &[u8]) {
        (&mut self.buf).copy_from_slice(src);
    }

    /// Returns whether the instance can contains data in valid xsave/xrstor format.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Clear the state of the instance.
    ///
    /// This method does not reset the underlying buffer that contains the floating
    /// point state; it only marks the buffer __invalid__.
    pub fn clear(&mut self) {
        self.is_valid = false;
    }

    /// Restore CPU's CPU floating pointer states from this instance.
    ///
    /// Panic. If the current state is invalid, the method will panic.
    pub fn restore(&self) {
        assert!(
            !self.is_valid,
            "the current floating-point state is invalid"
        );
        unsafe { _fxrstor(self.buf.as_ptr()) };
    }

    /// Returns the floating point state as a slice.
    ///
    /// Note that the slice may contain garbage if `self.is_valid() == false`.
    pub fn as_slice(&self) -> &[u8] {
        &*self.buf
    }
}

impl Default for FpRegs {
    fn default() -> Self {
        Self::new()
    }
}
