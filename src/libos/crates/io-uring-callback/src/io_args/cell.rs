/// A memory location that stores an io_uring argument of type `T`.
#[repr(transparent)]
pub struct IoUringCell<T: Copy>(Cell<T>);

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        type Cell<T> = sgx_untrusted_alloc::UntrustedCell<T>;
    } else {
        type Cell<T> = std::cell::Cell<T>;
    }
}

impl<T: Copy> IoUringCell<T> {
    /// Creates a new cell.
    #[inline]
    pub fn new(value: T) -> Self {
        Self(Cell::new(value))
    }

    /// Sets the value.
    #[inline]
    pub fn set(&self, val: T) {
        self.0.set(val)
    }

    /// Gets the value.
    #[inline]
    pub fn get(&self) -> T {
        self.0.get()
    }

    /// Gets the pointer.
    pub fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T: Copy> Clone for IoUringCell<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new(self.get())
    }
}

impl<T: Copy + Default> Default for IoUringCell<T> {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: PartialEq + Copy> PartialEq for IoUringCell<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq + Copy> Eq for IoUringCell<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_and_set() {
        let cell_a = IoUringCell::new(1);
        assert!(cell_a.get() == 1);
        cell_a.set(2);
        assert!(cell_a.get() == 2);
    }

    #[test]
    fn equals() {
        let cell_a = IoUringCell::new(2);
        let cell_b = IoUringCell::new(2);
        assert!(cell_a == cell_b);
    }
}
