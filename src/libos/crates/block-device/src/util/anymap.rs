//! AnyMap is a collection of heterogeneous types, storing one value of each type.

use alloc::collections::BTreeMap;
use alloc::fmt::{self, Debug};
use core::any::{Any as CoreAny, TypeId};

use super::unbox;
use crate::prelude::*;

/// A slightly extended version of `core::any::Any`.
pub trait Any: CoreAny + Debug + Send {}

impl<T: CoreAny + Debug + Send> Any for T {}

/// A collection of heterogeneous types, storing one value of each type. The
/// only requiremet for the types is to implement `Any`.
///
/// This is a simplified and specialized implementation of the `anymap` crate.
/// We do not choose to use the crate because 1) it is no longer maintained and
/// 2) it does not support the no_std environment.
pub struct AnyMap {
    map: BTreeMap<TypeId, Box<dyn Any>>,
}

impl AnyMap {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Insert a new value of a type, returning the old one if there is one.
    pub fn insert<T: Any + Sized>(&mut self, new_val: T) -> Option<T> {
        let type_id = TypeId::of::<T>();
        let boxed_new_val = Box::new(new_val) as _;
        self.map
            .insert(type_id, boxed_new_val)
            .map(|boxed_old_any| {
                // Safety. The target type T is guaranteed to match the actual type
                let boxed_old_val = unsafe { boxed_old_any.downcast_unchecked() };
                unbox(boxed_old_val)
            })
    }

    /// Get the value of a type.
    pub fn get<T: Any>(&self) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        self.map.get(&type_id).map(|any| {
            // Safety. The target type T is guaranteed to match the actual type
            unsafe { any.downcast_ref_unchecked() }
        })
    }

    /// Remove the value of a type.
    pub fn remove<T: Any + Sized>(&mut self) -> Option<T> {
        let type_id = TypeId::of::<T>();
        self.map.remove(&type_id).map(|boxed_old_any| {
            // Safety. The target type T is guaranteed to match the actual type
            let boxed_old_val = unsafe { boxed_old_any.downcast_unchecked() };
            unbox(boxed_old_val)
        })
    }
}

impl Debug for AnyMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.map.values().map(|v| &*v))
            .finish()
    }
}

/// An extension trait for `Any`.
trait DowncastUnchecked {
    /// Downcast a reference to `Self` to a reference to a concrete type `T`.
    ///
    /// # Safety
    ///
    /// The caller must be sure that the object pointed by `self` is indeed of type `T`.
    unsafe fn downcast_ref_unchecked<T: Any>(&self) -> &T;

    /// Downcast a boxed value of `Self` to a boxed value of type `T`.
    ///
    /// # Safety
    ///
    /// The caller must be sure that the object pointed by `self` is indeed of type `T`.
    unsafe fn downcast_unchecked<T: Any>(self: Box<Self>) -> Box<T>;
}

impl DowncastUnchecked for dyn Any {
    /// Downcast a reference to `Self` to a reference to a concrete type `T`.
    ///
    /// # Safety
    ///
    /// The caller must be sure that the object pointed by `self` is indeed of type `T`.
    unsafe fn downcast_ref_unchecked<T: Any>(&self) -> &T {
        &*(self as *const dyn Any as *const T)
    }

    /// Downcast a boxed value of `Self` to a boxed value of type `T`.
    ///
    /// # Safety
    ///
    /// The caller must be sure that the object pointed by `self` is indeed of type `T`.
    unsafe fn downcast_unchecked<T: Any>(self: Box<Self>) -> Box<T> {
        Box::from_raw(Box::into_raw(self) as *mut T)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove() {
        let mut anymap = AnyMap::new();
        assert!(anymap.insert(123_usize).is_none());
        assert!(anymap.get::<usize>() == Some(&123));
        assert!(anymap.remove::<usize>() == Some(123));
    }

    #[test]
    fn debug() {
        let mut anymap = AnyMap::new();
        assert!(anymap.insert(123_usize).is_none());
        assert!(anymap.insert(String::from("hello")).is_none());
        assert!(anymap.insert(0..10).is_none());
        println!("anymap = {:?}", anymap);
    }
}
