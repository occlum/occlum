// Convenient reexports for internal uses.
pub(crate) use block_device::BLOCK_SIZE;
pub(crate) use errno::prelude::*;
pub(crate) use std::sync::Arc;

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        pub(crate) use std::prelude::v1::*;
        pub(crate) use std::sync::{SgxMutex as Mutex};
        pub(crate) use std::untrusted::fs as fs;
    } else {
        pub(crate) use std::sync::{Mutex};
        pub(crate) use std::fs;
    }
}
