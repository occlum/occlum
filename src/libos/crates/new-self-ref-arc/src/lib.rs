//! A helper macro to initialize a self-referenced `Arc<T>`, where `T`
//! has a field named `weak_self: Weak<T>` that refers to its outer `Arc`.
//!
//! # Example
//!
//! Here is an example to show when and how to use the macro.
//!
//! ```rust
//! #![feature(get_mut_unchecked)]
//!
//! use std::sync::{Arc, Weak};
//!
//! use new_self_ref_arc::new_self_ref_arc;
//!
//! struct Dummy {
//!     weak_self: Weak<Dummy>,
//! }
//!
//! impl Dummy {
//!     pub fn new() -> Arc<Self> {
//!         let new_self = Dummy {
//!             weak_self: Weak::new(),
//!         };
//!         new_self_ref_arc!(new_self)
//!     }
//!
//!     // We can get the outer `Arc` even though we only have a `&self`
//!     // instead of `&Arc<Self>`. While in this specific example this may
//!     // seem to be trivial, it could be useful in more involved use cases
//!     // where getting a `&Arc<Self>` is not possible.
//!     pub fn clone_arc(&self) -> Arc<Self> {
//!         self.weak_self.upgrade().unwrap()
//!     }
//! }
//!
//! fn main() {
//!     let dummy = Dummy::new();
//!     let dummy2 = dummy.clone_arc();
//!     assert!(Arc::ptr_eq(&dummy, &dummy2));
//! }
//!```
#![feature(get_mut_unchecked)]

/// A helper macro to initialize a self-referenced `Arc<T>`. See the crate level doc.
#[macro_export]
macro_rules! new_self_ref_arc {
    ($val:expr) => {{
        let mut strong_self = Arc::new($val);
        let weak_self = Arc::downgrade(&strong_self);
        // Safety. This is safey since during the unsafe block the only two references
        // to the Arc, one strong reference and one weak, are not dereferenced.
        unsafe {
            Arc::get_mut_unchecked(&mut strong_self).weak_self = weak_self;
        }
        strong_self
    }};
}
