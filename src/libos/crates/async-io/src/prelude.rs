pub(crate) use std::sync::{Arc, Weak};

pub(crate) use spin::{Mutex, MutexGuard};

pub use errno::prelude::{Result, *};

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        pub(crate) use std::prelude::v1::*;
        pub(crate) use std::sync::{SgxRwLock as RwLock, SgxRwLockWriteGuard as RwLockWriteGuard};
    } else {
        pub(crate) use std::sync::{RwLock, RwLockWriteGuard};
    }
}
