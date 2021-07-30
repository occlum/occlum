//! Same as the standard `Arc`, except that it can be used as the key type of a hash table.
//!
//! # Motivation
//!
//! A type `K` is _keyable_ if it can be used as the key type for a hash map. Specifically,
//! according to the document of `std::collections::HashMap`, the type `K` must satisfy
//! the following properties.
//!
//! 1. It implements the `Eq` and `Hash` traits.
//! 2. The two values of `k1` and `k2` of type `K` equal to each other,
//! if and only if their hash values equal to each other.
//! 3. The hashes of a value of `k` of type `K` cannot change while it
//! is in a map.
//!
//! Sometimes we want to use `Arc<T>` as the key type for a hash map but cannot do so
//! since `T` does not satisfy the properties above. For example, a lot of types
//! do not or cannot implemennt the `Eq` trait. This is when `KeyableArc<T>` can come
//! to your aid.
//!
//! # Overview
//!
//! For any type `T`, `KeyableArc<T>` satisfies all the properties to be keyable.
//! This can be achieved easily and efficiently as we can simply use the address
//! of the data (of `T` type) of a `KeyableArc<T>` object in the heap to determine the
//! equality and hash of the `KeyableArc<T>` object. As the address won't change for
//! an immutable `KeyableArc<T>` object, the hash and equality also stay the same.
//!
//! # Usage
//!
//! Here is a basic example to how that `KeyableArc<T>` is keyable even when `T`
//! is not.
//!
//! ```rust
//! use std::collections::HashMap;
//! use std::sync::Arc;
//! use keyable_arc::KeyableArc;
//!
//! struct Dummy; // Does not implement Eq and Hash
//!
//! let map: HashMap<KeyableArc<Dummy>, String> = HashMap::new();
//! ```
//!
//! `KeyableArc` is a reference counter-based smart pointer, just like `Arc`.
//! So you can use `KeyableArc` the same way you would use `Arc`.
//!
//! ```rust
//! use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
//! use keyable_arc::KeyableArc;
//!
//! let key_arc0 = KeyableArc::new(AtomicU64::new(0));
//! let key_arc1 = key_arc0.clone();
//! assert!(key_arc0.load(Relaxed) == 0 && key_arc1.load(Relaxed) == 0);
//!
//! key_arc0.fetch_add(1, Relaxed);
//! assert!(key_arc0.load(Relaxed) == 1 && key_arc1.load(Relaxed) == 1);
//! ```
//!
//! # Differences from `Arc<T>`
//!
//! Notice how `KeyableArc` differs from standard smart pointers in determining equality?
//! Two `KeyableArc` objects are considered different even when their data have the same
//! value.
//!
//! ```rust
//! use keyable_arc::KeyableArc;
//!
//! let key_arc0 = KeyableArc::new(0);
//! let key_arc1 = key_arc0.clone();
//! assert!(key_arc0 == key_arc1);
//! assert!(*key_arc0 == *key_arc1);
//!
//! let key_arc1 = KeyableArc::new(0);
//! assert!(key_arc0 != key_arc1);
//! assert!(*key_arc0 == *key_arc1);
//! ```
//!
//! `KeyableArc<T>` is simply a wrapper of `Arc<T>. So converting between them
//! is zero cost.
//!
//! ```rust
//! use std::sync::Arc;
//! use keyable_arc::KeyableArc;
//!
//! let key_arc: KeyableArc<u32> = Arc::new(0).into();
//! let arc: Arc<u32> = KeyableArc::new(0).into();
//! ```
use std::borrow::Borrow;
use std::convert::AsRef;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// Same as the standard `Arc`, except that it can be used as the key type of a hash table.
#[repr(transparent)]
pub struct KeyableArc<T: ?Sized>(Arc<T>);

impl<T> KeyableArc<T> {
    #[inline]
    pub fn new(data: T) -> Self {
        Self(Arc::new(data))
    }
}

impl<T: ?Sized> KeyableArc<T> {
    #[inline]
    pub fn as_ptr(this: &Self) -> *const T {
        Arc::as_ptr(&this.0)
    }
}

impl<T: ?Sized> Deref for KeyableArc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &*self.0
    }
}

impl<T: ?Sized> AsRef<T> for KeyableArc<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized> Borrow<T> for KeyableArc<T> {
    #[inline]
    fn borrow(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized> From<Arc<T>> for KeyableArc<T> {
    #[inline]
    fn from(arc: Arc<T>) -> Self {
        Self(arc)
    }
}

impl<T: ?Sized> Into<Arc<T>> for KeyableArc<T> {
    #[inline]
    fn into(self) -> Arc<T> {
        self.0
    }
}

impl<T: ?Sized> PartialEq for KeyableArc<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.0) == Arc::as_ptr(&other.0)
    }
}

impl<T: ?Sized> Eq for KeyableArc<T> {}

impl<T: ?Sized> Hash for KeyableArc<T> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        Arc::as_ptr(&self.0).hash(s)
    }
}

impl<T: ?Sized> Clone for KeyableArc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
