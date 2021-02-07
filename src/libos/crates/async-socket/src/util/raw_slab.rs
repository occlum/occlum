#[cfg(feature = "sgx")]
use std::prelude::v1::*;
use std::ptr::NonNull;

/// Compared to `slab::Slab`, `RawSlab` gives more control by providing unsafe APIs.
pub struct RawSlab<T> {
    buf_ptr: NonNull<T>,
    buf_len: usize,
    // TODO: use bitmap
    free_indexes: Vec<usize>,
}

impl<T> RawSlab<T> {
    /// Create a slab allocator that can allocate as most `len` number of T objects.
    pub unsafe fn new(buf_ptr: *mut T, buf_len: usize) -> Self {
        let buf_ptr = NonNull::new(buf_ptr).unwrap();
        let free_indexes = (0..buf_len).into_iter().rev().collect();
        Self {
            buf_ptr,
            buf_len,
            free_indexes,
        }
    }

    /// Allocate an object.
    ///
    /// This method is semantically equivalent to
    /// ```no_run
    /// # unsafe fn call_malloc<T>() -> *mut libc::c_void {
    /// libc::malloc(std::mem::size_of::<T>())
    /// # }
    /// ```
    pub fn alloc(&mut self) -> Option<*mut T> {
        let free_index = match self.free_indexes.pop() {
            None => return None,
            Some(free_index) => free_index,
        };

        let ptr = unsafe { self.buf_ptr.as_ptr().add(free_index) };
        Some(ptr)
    }

    /// Deallocate an object.
    ///
    /// This method is semantically equivalent to
    /// ```no_run
    /// # let ptr = std::ptr::null_mut();
    /// # unsafe {
    /// libc::free(ptr);
    /// # }
    /// ```
    /// where ptr is a pointer to an object of `T` that
    /// is previously allocated by this allocator.
    ///
    /// Memory safety. The user carries the same responsibility as he would do
    /// with C's free. So use it carefully.
    pub unsafe fn dealloc(&mut self, ptr: *mut T) {
        let index = ptr.offset_from(self.buf_ptr.as_ptr()) as usize;
        debug_assert!(self.buf_ptr.as_ptr().add(index) == ptr);
        self.free_indexes.push(index);
    }

    /// Returns the max number of objects that can be allocated.
    pub fn capacity(&self) -> usize {
        self.buf_len
    }

    /// Returns the number of allocated objects.
    pub fn allocated(&self) -> usize {
        self.capacity() - self.free_indexes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let capacity = 1024;
        let mut vec: Vec<i32> = Vec::with_capacity(capacity);
        assert_eq!(capacity, vec.capacity());

        let mut slab = unsafe { RawSlab::new(vec.as_mut_ptr(), vec.capacity()) };
        assert_eq!(slab.capacity(), vec.capacity());
        assert_eq!(slab.allocated(), 0);

        let mut ptr_vec: Vec<*mut i32> = Vec::with_capacity(capacity);
        for i in 0..capacity {
            let entry = slab.alloc();
            assert_eq!(entry.is_some(), true);
            ptr_vec.push(entry.unwrap());
        }

        let entry = slab.alloc();
        assert_eq!(entry.is_none(), true);
        assert_eq!(slab.allocated(), capacity);

        for i in 0..capacity {
            unsafe {
                slab.dealloc(ptr_vec[i]);
            }
        }
        assert_eq!(slab.allocated(), 0);

        let value = slab.alloc().unwrap();
        unsafe {
            *value = 1;
        }
        assert_eq!(slab.allocated(), 1);
        unsafe {
            slab.dealloc(value);
        }
        assert_eq!(slab.allocated(), 0);
    }
}
