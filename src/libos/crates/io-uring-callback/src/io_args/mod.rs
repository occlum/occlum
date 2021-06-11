//! Memory locations to store io_uring arguments.
//!
//! There are two properties that make arguments for io_uring special.
//!
//! First, they break Rust's principle of "Aliasing XOR Mutability" as the kernel
//! side may modify io_uring arguments concurrently. Thus, it would be a good idea to
//! mark these arguments with `UnsafeCell` so that Rust compiler won't make false
//! assumptions about them.
//!
//! Second, in SGX environments, Rust values are by default stored in the trusted
//! memory, which cannot be touched by the untrusted Linux kernel and naturally are
//! not suitable to serve as arguments for io_uring commands. The types provided in
//! this module automatically chooses the right memory depending on whether the
//! environment is SGX or not.

mod array;
mod cell;

pub use self::array::IoUringArray;
pub use self::cell::IoUringCell;
