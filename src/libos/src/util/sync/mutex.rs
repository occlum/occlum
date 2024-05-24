use std::{
    cell::UnsafeCell,
    hint,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, AtomicU32},
};

use alloc::{boxed::Box, fmt};
use atomic::Ordering;

use crate::process::{futex_wait, futex_wake};

#[derive(Default)]
pub struct Mutex<T: ?Sized> {
    inner: Box<MutexInner>,
    value: UnsafeCell<T>,
}

unsafe impl<T: Send + ?Sized> Sync for Mutex<T> {}
unsafe impl<T: Send + ?Sized> Send for Mutex<T> {}

pub struct MutexGuard<'a, T: ?Sized + 'a> {
    inner: &'a Mutex<T>,
}

impl<T: ?Sized> !Send for MutexGuard<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for MutexGuard<'_, T> {}

impl<T> Mutex<T> {
    #[inline]
    pub fn new(val: T) -> Mutex<T> {
        Self {
            value: UnsafeCell::new(val),
            inner: Box::new(MutexInner::new()),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock();
        MutexGuard { inner: self }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.inner.try_lock().then(|| MutexGuard { inner: self })
    }

    #[inline]
    pub fn unlock(guard: MutexGuard<'_, T>) {
        drop(guard)
    }

    #[inline]
    unsafe fn force_unlock(&self) {
        self.inner.force_unlock()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    #[inline]
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.value.into_inner()
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.value.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner.value.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        unsafe {
            self.inner.force_unlock();
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => write!(f, "Mutex {{ value: ")
                .and_then(|()| (*guard).fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => {
                write!(f, "Mutex {{ <locked> }}")
            }
        }
    }
}

#[derive(Default)]
struct MutexInner {
    /// 0: unlocked
    /// 1: locked, no other threads waiting
    /// 2: locked, and other threads waiting (contended)
    lock: AtomicU32,
}

impl MutexInner {
    #[inline]
    pub fn new() -> MutexInner {
        Self {
            lock: AtomicU32::new(0),
        }
    }

    #[inline]
    pub fn lock(&self) {
        if self
            .lock
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            self.lock_contended();
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        self.lock
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    unsafe fn force_unlock(&self) {
        if self.lock.swap(0, Ordering::Release) == 2 {
            self.wake();
        }
    }

    #[cold]
    fn lock_contended(&self) {
        let mut state = self.spin();

        if state == 0 {
            match self
                .lock
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return, // Locked!
                Err(s) => state = s,
            }
        }

        loop {
            if state != 2 && self.lock.swap(2, Ordering::Acquire) == 0 {
                return;
            }

            // Wait for the futex to change state, assuming it is still 2.
            let ret = futex_wait(&self.lock as *const _ as *const i32, 2, &None);

            // Spin again after waking up.
            state = self.spin();
        }
    }

    #[inline]
    fn spin(&self) -> u32 {
        let mut spin = 1000;
        loop {
            // We only use `load` (and not `swap` or `compare_exchange`)
            // while spinning, to be easier on the caches.
            let state = self.lock.load(Ordering::Relaxed);

            // We stop spinning when the mutex is unlocked (0),
            // but also when it's contended (2).
            if state != 1 || spin == 0 {
                return state;
            }

            core::hint::spin_loop();
            spin -= 1;
        }
    }

    #[cold]
    fn wake(&self) {
        futex_wake(&self.lock as *const _ as *const i32, 1);
    }
}
