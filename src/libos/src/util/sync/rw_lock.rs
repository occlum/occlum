use super::*;
use errno::prelude::Result;
use spin::RwLock;

// Implement a wrapper for spin RwLock for fast replacement.
// Previously, we use SgxRwLock, but running flink demo failure indicates there must be something wrong with it.
// After we fix the bug or we implement our own read-write lock, this file can be deleted.

#[derive(Debug, Default)]
pub struct RwLockWrapper<T: ?Sized>(RwLock<T>);

impl<T> RwLockWrapper<T> {
    pub fn new(user_data: T) -> RwLockWrapper<T> {
        RwLockWrapper(RwLock::new(user_data))
    }

    pub fn read(&self) -> Result<RwLockReadGuard<T>> {
        Ok(self.0.read())
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<T>> {
        Ok(self.0.write())
    }
}
