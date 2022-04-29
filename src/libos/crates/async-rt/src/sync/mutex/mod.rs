use super::*;

mod inner;

use inner::MutexInner;
/// An asynchronous mutex type.
///
/// This is simillar to `std::sync::Mutex` but with the following differences:
/// - `lock` method is asynchronous and will not block
/// - the `MutexGuard` can be held across `await` calls
pub struct Mutex<T: ?Sized> {
    inner: MutexInner,
    data: UnsafeCell<T>,
}

/// Mutex can be used across threads as long as T is `Send`.
unsafe impl<T> Send for Mutex<T> where T: ?Sized + Send {}
unsafe impl<T> Sync for Mutex<T> where T: ?Sized + Send {}

/// A handle to a held mutex. This can be used across `await` calls because
/// it is `Send`.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
}

/// Automatically `Send` marked for MutexGuard.
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<T> Mutex<T> {
    pub fn new(t: T) -> Mutex<T> {
        Mutex {
            inner: MutexInner::new(),
            data: UnsafeCell::new(t),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Async method to lock the mutex
    pub async fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock().await;
        MutexGuard::new(self)
    }

    /// Try acquiring the lock without blocking
    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>> {
        self.inner.try_lock()?;
        Ok(MutexGuard::new(self))
    }

    /// Consume the mutex to get inner T
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        let Mutex {
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

impl<T: core::fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Mutex");
        match self.try_lock() {
            Ok(guard) => debug.field("data", &&*guard),
            Err(_) => debug.field("data", &"<locked>"),
        };
        debug.finish()
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    fn new(lock: &'a Mutex<T>) -> MutexGuard<'a, T> {
        MutexGuard { lock }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.inner.unlock();
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
