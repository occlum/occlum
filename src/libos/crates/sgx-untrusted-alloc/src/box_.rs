use crate::untrusted_allocator::Allocator;
use std::fmt::Debug;
use std::mem::{align_of, size_of};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

lazy_static! {
    static ref UNTRUSTED_MEM_INSTANCE: Allocator = Allocator::new();
}

use crate::MaybeUntrusted;

/// A memory location on the heap in untrusted memory.
///
/// `UntrustedBox<T>` Behaves similar to the standard `Box<T>`, except that
/// it requires that the type bound of `T: MaybeUntrusted`. This is a safety
/// measure to avoid potential misuses.
pub struct UntrustedBox<T: ?Sized> {
    ptr: NonNull<T>,
}

impl<T> Debug for UntrustedBox<T>
where
    T: ?Sized + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Untrusted T")
            .field("T", unsafe { &self.ptr.as_ref() })
            .finish()
    }
}

impl<T: MaybeUntrusted> UntrustedBox<T> {
    /// Creates a value of `T` on the heap in untrusted memory.
    pub fn new(val: T) -> Self {
        let mut new_self = Self::new_uninit();
        *new_self = val;
        new_self
    }

    /// Creates an _uninitialized_ value of `T` on the heap in untrusted memory.
    pub fn new_uninit() -> Self {
        let ptr = {
            let raw_ptr = unsafe {
                UNTRUSTED_MEM_INSTANCE
                    .alloc(size_of::<T>(), None)
                    .expect("memory allocation failure")
            } as *mut T;
            assert!(raw_ptr != std::ptr::null_mut());
            assert!((raw_ptr as usize) % align_of::<T>() == 0);
            NonNull::new(raw_ptr).unwrap()
        };
        Self { ptr }
    }
}

impl<T: MaybeUntrusted + Copy> UntrustedBox<[T]> {
    /// Creates a slice of `T` on the heap in untrusted memory.
    ///
    /// Note that the pointer and length of the slice is still kept in trusted memory;
    /// only the pointer refers to untrusted memory. Thus, there is no risk of buffer
    /// overflow.
    pub fn new_slice(slice: &[T]) -> Self {
        let mut uninit_slice = Self::new_uninit_slice(slice.len());
        uninit_slice.copy_from_slice(slice);
        uninit_slice
    }
}

impl<T: MaybeUntrusted> UntrustedBox<[T]> {
    /// Creates an uninitialized slice of `T` on the heap in untrusted memory.
    pub fn new_uninit_slice(len: usize) -> Self {
        let ptr = {
            let total_bytes = size_of::<T>() * len;
            let raw_slice_ptr = unsafe {
                UNTRUSTED_MEM_INSTANCE
                    .alloc(total_bytes, None)
                    .expect("memory allocation failure")
            } as *mut T;
            assert!(raw_slice_ptr != std::ptr::null_mut());
            assert!((raw_slice_ptr as usize) % align_of::<T>() == 0);
            let untrusted_slice = unsafe { std::slice::from_raw_parts_mut(raw_slice_ptr, len) };
            // For DST types like slice, NonNull is now a fat pointer.
            NonNull::new(untrusted_slice as _).unwrap()
        };
        Self { ptr }
    }
}

impl<T: ?Sized> UntrustedBox<T> {
    /// Gets an immutable pointer of the value on the untrusted memory.
    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }

    /// Gets a mutable pointer of the value on the untrusted memory.
    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

impl<T: ?Sized> Drop for UntrustedBox<T> {
    fn drop(&mut self) {
        unsafe {
            UNTRUSTED_MEM_INSTANCE.free(self.as_mut_ptr() as *mut u8);
        }
    }
}

impl<T: ?Sized> Deref for UntrustedBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for UntrustedBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: MaybeUntrusted + Default> Default for UntrustedBox<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: MaybeUntrusted + Clone> Clone for UntrustedBox<T> {
    fn clone(&self) -> Self {
        Self::new(self.deref().clone())
    }
}

unsafe impl<T: ?Sized + Send> Send for UntrustedBox<T> {}
unsafe impl<T: ?Sized + Sync> Sync for UntrustedBox<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    struct Point {
        x: usize,
        y: usize,
    }
    unsafe impl MaybeUntrusted for Point {}

    #[test]
    fn with_i32() {
        let mut untrusted_i32 = UntrustedBox::new(0i32);
        assert!(*untrusted_i32 == 0);
        *untrusted_i32 = 1;
        assert!(*untrusted_i32 == 1);
        drop(untrusted_i32);
    }

    #[test]
    fn with_point() {
        let mut untrusted_point = UntrustedBox::new(Point { x: 0, y: 0 });
        assert!(untrusted_point.x == 0 && untrusted_point.y == 0);
        untrusted_point.x += 10;
        untrusted_point.y += 20;
        assert!(untrusted_point.x == 10 && untrusted_point.y == 20);
        drop(untrusted_point);
    }

    #[test]
    fn with_array() {
        let mut untrusted_array = UntrustedBox::new([0u8, 1, 2, 3]);
        untrusted_array
            .iter()
            .enumerate()
            .for_each(|(pos, i)| assert!(pos as u8 == *i));

        for i in untrusted_array.iter_mut() {
            *i = 0;
        }
        untrusted_array.iter().for_each(|i| assert!(*i == 0));
    }

    #[test]
    fn with_slice() {
        let len = 4;
        let mut untrusted_slice: UntrustedBox<[i32]> = UntrustedBox::new_uninit_slice(len);
        assert!(untrusted_slice.len() == len);
        untrusted_slice[1] = 123;
        assert!(untrusted_slice[1] == 123);
    }
}
