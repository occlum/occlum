//! User-friendly error handling with build-in support for POSIX errno.
//!
//! This crate extends Rust's standard error handling with the abilities of
//! reporting error locations, providing backtrace information, and unifying
//! all types of errors with POSIX errno.
//!
//! # Motivation
//!
//! While the built-in error handling mechanism of Rust is undoubtedly superior
//! than that of a traditional system programming language (e.g., C/C++), it
//! is _not perfect_.
//!
//! First, trait `std::error::Error` does not provide any means to
//! record the location of the source code that triggers an error, leading to
//! a slow process of diagnosing errors or bugs.
//!
//! Second, while the `Error` trait (which has a `cause` method)
//! supports backtrace in theory, it is inconvenient---in practice---to implement
//! backtrace. This is because the users still need to manually write the concrete
//! implementation that stores the cause for every error struct.
//!
//! Third, one challenging aspect of error handling in Rust is
//! dealing with the various types of errors. The standard library
//! defines errors like `std::io::Error`, `std::fmt::Error`, `std::str::Utf8Error`, etc.
//! Not to mention the error types defined by third-party libraries.
//! To make it even worse, we, as OS writers, have to convert all these errors
//! into POSIX errno eventually.
//!
//! To cope with the issues above, this crate extends Rust's standard error
//! handling mechanism. Specifically, it aims at the following design goals:
//!
//! * **Fast diagnose** (e.g., reporting the backtrace and the code location of an error).
//! * **First-class POSIX errno** (e.g., every error has an errno).
//! * **Zero-overhead abstraction** (e.g., no heap allocation unless absolutely necesary).
//! * **Ergonomic grammar** (e.g., use macros to avoid writing code manually).
//! * **Compatibility with `no_std`**.
//!
//! # How to Use
//!
//! ## Basic Usage
//!
//! The simplest usage involves just one macro---`errno!`.
//! See the sample code below:
//! ```rust
//! use errno::prelude::*;
//!
//! fn return_err() -> Result<()> {
//!    Err(errno!(EINVAL, "the root error"))
//! }
//!
//! # fn main() {
//! if let Err(e) = return_err() {
//!     println!("{}", e);
//! }
//! # }
//! ```
//! which prints something like
//! ```text
//! EINVAL (#22, Invalid argument): the root error [line = 45, file = src/lib.rs]
//! ```
//! Note that the specific line and file of source code that generates the error
//! is printed. This facilitates diagnosing errors.
//!
//! ## Backtrace
//!
//! A more interesting usage is to print the backtrace of an error. To create
//! the chains of errors, `std::result::Result` is extended with a new method
//! named `cause_err`. If the result is `Ok`, the method does nothing; otherwise,
//! this method executes a user-given closure to output a new error whose cause
//! is the error contained in the result. The method consumes the current result
//! and generates a new result that contains the new error. The two errors are
//! chained. More calls to `cause_err` form deeper backtraces.
//!
//! See the sample code below:
//! ```rust
//! use errno::prelude::*;
//!
//! fn return_err() -> Result<()> {
//!     Err(errno!(EINVAL, "the root error"))
//! }
//!
//! fn cause_err() -> Result<()> {
//!     return_err()
//!         .cause_err(|_e| errno!(EIO, "another error"))
//! }
//!
//! # fn main() {
//! if let Err(e) = cause_err() {
//!     println!("{}", e.backtrace());
//! }
//! # }
//! ```
//! which prints something like
//! ```text
//! EIO (#5, I/O error): another error [line = 71, file = src/lib.rs]
//!     Caused by EINVAL (#22, Invalid argument): the root error [line = 68, file = src/lib.rs]
//! ```
//!

#![feature(allocator_api)]
// Use no_std and alloc crate except when given std feature or during test.
#![cfg_attr(not(any(feature = "std", test, doctest)), no_std)]
#[cfg(not(any(feature = "std", test, doctest)))]
extern crate alloc;
// Use Rust SGX SDK's std when given SGX feature.
#[cfg(feature = "sgx")]
extern crate sgx_tstd as std;

mod backtrace;
mod errno;
mod error;
mod macros;
pub mod prelude;
mod to_errno;

pub use self::backtrace::{ErrorBacktrace, ResultExt};
pub use self::errno::Errno;
pub use self::error::{Error, ErrorLocation};
pub use self::to_errno::ToErrno;

pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn convert_std_io_error() -> Result<()> {
        use std::io::{BufWriter, Write};
        let mut buf_writer = BufWriter::new(Vec::<u8>::new());
        // std::io::Error can be converted crate::Error implicitly
        buf_writer.write("foo".as_bytes())?;
        Ok(())
    }

    #[test]
    fn convert_std_ffi_nul_error() -> Result<()> {
        use std::ffi::CString;
        // std::ffi::NulError can be converted crate::Error implicitly
        let _ = CString::new(b"foo".to_vec())?;
        Ok(())
    }
}
