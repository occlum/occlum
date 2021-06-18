use super::*;
use core::sync::atomic::{AtomicU64, Ordering};

/// A unique ID.
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
