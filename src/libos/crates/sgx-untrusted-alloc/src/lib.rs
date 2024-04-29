//! Allocation and access of _untrusted_ memory in a _safe_ way.
//!
//! # Usage
//!
//! ## Basics
//!
//! Suppose you have a data structure named `AcceptReq`
//! ```rust
//! struct AcceptReq {
//!     addr: libc::sockaddr_storage,
//!     addr_len: libc::socklen_t,
//! }
//! ```
//! which is intended to be used as an untrusted buffer shared
//! with the host OS to pass arguments of the accept system call.
//! And we assume that this buffer must be present during the lifetime
//! of a listening socket. So it must be allocated on the heap in
//! untrusted memory. So how to do it?
//!
//! With this crate, it takes two steps.
//!
//! 1. Implement the [`MaybeUntrusted`] marker trait for the data structure.
//!
//! ```rust
//! use sgx_untrusted_alloc::MaybeUntrusted;
//! # struct AcceptReq;
//!
//! unsafe impl MaybeUntrusted for AcceptReq { }
//! ```
//!
//! By implementing this trait, you are claiming: "I am fully aware of the
//! security risks in communicating with the host through _untrusted,
//! shared data structures_. I know that an attacker may peek or tamper with
//! the data structure at any possible timing or in an arbitrary way.
//! I will be very careful. And I am good to go."
//!
//! 2. You can now allocate the data structure in untrusted heap with
//! [`UntrustedBox`], which is similar to the standard `Box` albeit for the
//! untrusted memory.
//!
//! ```rust
//! # use sgx_untrusted_alloc::MaybeUntrusted;
//! # struct AcceptReq;
//! # unsafe impl MaybeUntrusted for AcceptReq { }
//! #
//! use sgx_untrusted_alloc::UntrustedBox;
//!
//! let accept_req: UntrustedBox<AcceptReq> = UntrustedBox::new_uninit();
//! ```
//!
//! Note that the convenient constructor method `UntrustedBox::<T>::new_uninit`
//! creates an _uninitialized_ instance of `T` on untrusted heap.
//! Alternatively, you can create an _initialized_ instance with `UntrustedBox::new`.
//!
//! ## Arrays and slices
//!
//! You can also use `UntrustedBox` to allocate arrays (`[T; N]`) or
//! slices (`[T]`) on untrusted heap as long as the trait bound of `T: MaybeUntrusted`
//! is held.
//!
//! ```rust
//! use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};
//!
//! let untrusted_array: UntrustedBox<[u8; 4]> = UntrustedBox::new_uninit();
//!
//! let untrusted_slice: UntrustedBox<[u8]> = UntrustedBox::new_uninit_slice(4);
//! ```
//!
//! Both `untrusted_array` and `untrusted_slice` above consist of four `u8` integers.

#![cfg_attr(feature = "sgx", no_std)]
#![feature(linked_list_remove)]

#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_tstd as std;

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate lazy_static;
extern crate intrusive_collections;
#[macro_use]
extern crate log;
extern crate spin;

mod maybe_untrusted;
pub use maybe_untrusted::MaybeUntrusted;

mod box_;
pub use box_::UntrustedBox;

mod prelude;
mod untrusted_allocator;
