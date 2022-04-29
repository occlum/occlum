use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use errno::prelude::*;
use std::convert::{TryFrom, TryInto};
use std::hint;
use std::sync::atomic::Ordering;

mod mutex;
mod rwlock;

pub use mutex::{Mutex, MutexGuard};
pub use rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
