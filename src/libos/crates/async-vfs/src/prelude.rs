// Convenient reexports for internal uses.
pub(crate) use errno::prelude::*;
pub(crate) use std::sync::{Arc, Weak};

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        pub(crate) use std::prelude::v1::*;
        pub(crate) use std::sync::{SgxMutex as Mutex, SgxRwLock as RwLock};
    } else {
        pub(crate) use std::sync::{Mutex, RwLock};
    }
}
