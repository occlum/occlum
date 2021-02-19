#[cfg(not(feature = "sgx"))]
pub(crate) use std::sync::{Arc, Mutex, MutexGuard, Weak};

#[cfg(feature = "sgx")]
pub(crate) use std::prelude::v1::*;
#[cfg(feature = "sgx")]
pub(crate) use std::sync::{Arc, SgxMutex as Mutex, SgxMutexGuard as MutexGuard, Weak};

pub use errno::prelude::{Result, *};
