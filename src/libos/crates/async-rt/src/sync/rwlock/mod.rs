use super::*;

mod inner;

use inner::RwLockInner;

/// A readers-writer lock implementation which allows many readers or at most one writer at the same time.
pub struct RwLock<T: ?Sized> {
    inner: RwLockInner,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
// RwLock doesn't need T to be Sync here because RwLock doesn't access T directly.
unsafe impl<T: ?Sized + Send> Sync for RwLock<T> {}

/// The RAII guard for read that can be held by many readers
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
}

/// The read guard can be obtained by different threads. `Send` is marked automatically.
unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

/// The RAII gurad for write that can be held by only one writer
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
}

// The write guard can be obtained by different threads. `Send` is marked automatically.
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

impl<T> RwLock<T> {
    pub fn new(t: T) -> RwLock<T> {
        RwLock {
            inner: RwLockInner::new(),
            data: UnsafeCell::new(t),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Async method to acquire the read lock
    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        self.inner.read().await.unwrap();
        RwLockReadGuard::new(self)
    }

    /// Try acuiring the read lock without blocking
    pub fn try_read(&self) -> Result<RwLockReadGuard<'_, T>> {
        self.inner.try_read()?;
        Ok(RwLockReadGuard::new(self))
    }

    /// Async method to acquire the write lock
    pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.inner.write().await.unwrap();
        RwLockWriteGuard::new(self)
    }

    /// Try acuiring the write lock without blocking
    pub fn try_write(&self) -> Result<RwLockWriteGuard<'_, T>> {
        self.inner.try_write()?;
        Ok(RwLockWriteGuard::new(self))
    }

    /// Consume the lock to get inner T
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        let RwLock {
            inner: _inner,
            data,
        } = self;

        data.into_inner()
    }

    /// Get a mutable reference to the inner data
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

impl<T: ?Sized + Default> Default for RwLock<T> {
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

impl<T: core::fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("RwLock");
        match self.try_read() {
            Ok(guard) => debug.field("data", &&*guard),
            Err(_) => debug.field("data", &"<locked>"),
        };
        debug.finish()
    }
}

impl<'a, T: ?Sized> RwLockReadGuard<'a, T> {
    fn new(lock: &'a RwLock<T>) -> RwLockReadGuard<'a, T> {
        RwLockReadGuard { lock }
    }
}

impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    pub fn new(lock: &'a RwLock<T>) -> RwLockWriteGuard<'a, T> {
        RwLockWriteGuard { lock }
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
