use super::*;

mod rw_lock;

pub use self::rw_lock::RwLockWrapper as RwLock;
pub use spin::{RwLockReadGuard, RwLockWriteGuard};
