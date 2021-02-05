#[cfg(feature = "sgx")]
use std::prelude::v1::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// The unique id of an object.
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct ObjectId(u64);

impl ObjectId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self(id)
    }

    pub const fn null() -> Self {
        Self(0)
    }
}
