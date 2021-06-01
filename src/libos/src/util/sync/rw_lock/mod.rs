mod inner;

use super::*;

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::{fmt, mem, ptr};
use inner::RwLockInner;
use std::boxed::Box;

// A readers-writer lock with the same methods as std::sync::RwLock except is_poisoned.
// It allows many readers or at most one writer at the same time.
// TODO: Add poison support
pub struct RwLock<T: ?Sized> {
    inner: Box<RwLockInner>,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

// The RAII guard for read that can be held by many readers
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
}

// The read guard can be obtained by different threads
// but not sent from one thread to another thread
impl<T: ?Sized> !Send for RwLockReadGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

// The RAII gurad for write that can be held by only one writer
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
}

// The write guard can be obtained by different threads
// but not sent from one thread to another thread
impl<T: ?Sized> !Send for RwLockWriteGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

impl<T> RwLock<T> {
    pub fn new(t: T) -> RwLock<T> {
        RwLock {
            inner: Box::new(RwLockInner::new()),
            data: UnsafeCell::new(t),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    pub fn read(&self) -> Result<RwLockReadGuard<'_, T>> {
        self.inner.read()?;
        RwLockReadGuard::new(self)
    }

    pub fn try_read(&self) -> Result<RwLockReadGuard<'_, T>> {
        self.inner.try_read()?;
        RwLockReadGuard::new(self)
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, T>> {
        unsafe {
            self.inner.write()?;
            RwLockWriteGuard::new(self)
        }
    }

    pub fn try_write(&self) -> Result<RwLockWriteGuard<'_, T>> {
        unsafe {
            self.inner.try_write()?;
            RwLockWriteGuard::new(self)
        }
    }

    pub fn into_inner(self) -> Result<T>
    where
        T: Sized,
    {
        unsafe {
            let (inner, data) = {
                let RwLock {
                    ref inner,
                    ref data,
                } = self;
                (ptr::read(inner), ptr::read(data))
            };
            mem::forget(self);
            inner.destroy();
            drop(inner);

            Ok(data.into_inner())
        }
    }

    pub fn get_mut(&mut self) -> Result<&mut T> {
        let data = unsafe { &mut *self.data.get() };
        Ok(data)
    }
}

// Use may_dangle to assert not to access T
unsafe impl<#[may_dangle] T: ?Sized> Drop for RwLock<T> {
    fn drop(&mut self) {
        self.inner.destroy().unwrap();
    }
}

impl<T: ?Sized + Default> Default for RwLock<T> {
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

impl<T: core::fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLock")
            .field("inner", unsafe { &self.inner })
            .field("data", unsafe { &(*self.data.get()) })
            .finish()
    }
}

impl<'a, T: ?Sized> RwLockReadGuard<'a, T> {
    pub fn new(lock: &'a RwLock<T>) -> Result<RwLockReadGuard<'a, T>> {
        Ok(RwLockReadGuard { lock })
    }
}

impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    pub fn new(lock: &'a RwLock<T>) -> Result<RwLockWriteGuard<'a, T>> {
        Ok(RwLockWriteGuard { lock })
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.inner.read_unlock().unwrap();
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.inner.write_unlock().unwrap();
    }
}
