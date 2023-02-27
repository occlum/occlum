use crate::prelude::*;

pub mod anymap;
pub mod block_range;
pub mod test;

/// Equivalent to `Box::into_inner`. The latter method is not available in
/// the version of Rust toolchain that we currently use.
pub fn unbox<T: Sized>(value: Box<T>) -> T {
    *value
}

pub(crate) const fn align_down(x: usize, align: usize) -> usize {
    debug_assert!(align >= 2 && align.is_power_of_two());
    (x / align) * align
}

pub(crate) const fn align_up(x: usize, align: usize) -> usize {
    debug_assert!(align >= 2 && align.is_power_of_two());
    ((x + align - 1) / align) * align
}
