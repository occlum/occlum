#![allow(unused)]

#![crate_name = "occlum_rs"]
#![crate_type = "staticlib"]

#![cfg_attr(not(target_env = "sgx"), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]
#![feature(allocator_api)]
#![feature(integer_atomics)]
#![feature(range_contains)]

extern crate sgx_types;
#[cfg(not(target_env = "sgx"))]
#[macro_use]
extern crate sgx_tstd as std;
extern crate sgx_trts;
extern crate xmas_elf;
#[macro_use]
extern crate lazy_static;

use std::ffi::CStr; // a borrowed C string
use std::backtrace::{self, PrintFormat};
use std::panic;
use sgx_types::*;
use sgx_trts::libc;

#[macro_use]
mod prelude;
mod entry;
mod errno;
mod fs;
mod process;
mod syscall;
mod vm;
mod util;
mod time;

use prelude::*;

// Export system calls
pub use syscall::*;
// Export ECalls
pub use entry::*;
