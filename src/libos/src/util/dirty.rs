/// Dirty is a wrapper type that remembers whether the internal object has been
/// borrowed mutably.
use std::fmt;

pub struct Dirty<T> {
    inner: T,
    dirty: bool,
}

impl<T> Dirty<T> {
    pub fn new(inner: T) -> Self {
        let dirty = false;
        Self { inner, dirty }
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn unwrap(self) -> T {
        self.inner
    }
}

impl<T: fmt::Debug> fmt::Debug for Dirty<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Dirty")
            .field("inner", &self.inner)
            .field("dirty", &self.dirty)
            .finish()
    }
}

impl<T> AsRef<T> for Dirty<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for Dirty<T> {
    fn as_mut(&mut self) -> &mut T {
        self.dirty = true;
        &mut self.inner
    }
}

impl<T: Copy> Copy for Dirty<T> {}

impl<T: Clone> Clone for Dirty<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            dirty: self.dirty,
        }
    }
}
