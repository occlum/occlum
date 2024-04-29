//! Assign a unique and immutable ID.
//!
//! Some types do not have a natural implementation for `PartialEq` or `Hash`.
//! In such cases, it can be convenient to assign an unique ID for each instance
//! of such types and use the ID to implement `PartialEq` or `Hash`.
//!
//! An ID have a length of 64-bit.

#![cfg_attr(not(any(test, doctest)), no_std)]

use core::sync::atomic::{AtomicU64, Ordering};

/// A unique id.
#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
#[repr(transparent)]
pub struct ObjectId(u64);

impl ObjectId {
    /// Create a new unique ID.
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        // Make sure that we can detect the overflow of id even in face of
        // (extremely) concurrent addition on NEXT_ID.
        assert!(id <= u64::max_value() / 2);

        Self(id)
    }

    /// Return a special "null" ID.
    ///
    /// Note that no ID created by `ObjectId::new()` will be equivalent to the
    /// null ID.
    pub const fn null() -> Self {
        Self(0)
    }

    /// Get the ID value as `u64`.
    pub const fn get(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique() {
        let id0 = ObjectId::new();
        let id1 = ObjectId::new();
        assert!(id0 != id1);
        assert!(id0.get() < id1.get());
    }

    #[test]
    fn non_null() {
        let id0 = ObjectId::new();
        assert!(id0 != ObjectId::null());
    }
}
