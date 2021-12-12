use crate::prelude::*;

pub mod anymap;
pub mod test;

/// Equivalent to `Box::into_inner`. The latter method is not available in
/// the version of Rust toolchain that we currently use.
pub fn unbox<T: Sized>(value: Box<T>) -> T {
    *value
}
