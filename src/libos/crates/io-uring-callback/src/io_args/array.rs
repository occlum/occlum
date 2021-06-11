/// A memory location that stores an array of elements of `T` type.
///
/// For the sake of efficiency, each element is not initialized automatically.
pub struct IoUringArray<T: Copy>(Array<T>);

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        type Array<T> = sgx_untrusted_alloc::UntrustedArray<T>;
    } else {
        use std::mem::MaybeUninit;
        use std::cell::Cell;

        type Array<T> = std::vec::Vec<Cell<MaybeUninit<T>>>;
    }
}

impl<T: Copy> IoUringArray<T> {
    /// Create an array with the specified number of elements of type `T`.
    pub fn with_capacity(capacity: usize) -> Self {
        let array = Array::with_capacity(capacity);
        Self(array)
    }

    /// Get the value of an element.
    ///
    /// # Safety
    /// 
    /// The caller must ensure that the element at the given position has been initialized.
    pub unsafe fn get(&self, index: usize) -> T {
        *self.pos_ptr(index)
    }

    /// Set the value of an element.
    pub fn set(&self, index: usize, val: T) {
        unsafe {
            *self.pos_ptr(index) = val;
        }
    }

    /// Returns the capacity of the array.
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Returns the base pointer of the array.
    pub fn as_ptr(&self) -> *mut T {
        cfg_if::cfg_if! {
            if #[cfg(feature = "sgx")] {
                self.0.as_ptr()
            } else {
                self.0.as_ptr() as _
            }
        }
    }

    fn pos_ptr(&self, index: usize) -> *mut T {
        assert!(index < self.capacity());
        unsafe { self.as_ptr().add(index) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_and_set() {
        let array = IoUringArray::with_capacity(4);
        (0..4).for_each(|idx| {
            let val = idx * idx;
            array.set(idx, val);
        });
        (0..4).for_each(|idx| {
            let actual_val = unsafe { array.get(idx) };
            let expected_val = idx * idx;
            assert!(actual_val == expected_val);
        });
    }
}
