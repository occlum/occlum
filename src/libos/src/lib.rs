#![allow(unused)]
#![crate_name = "occlum_libos_core_rs"]
#![crate_type = "staticlib"]
#![cfg_attr(not(target_env = "sgx"), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]
#![feature(allocator_api)]
#![feature(core_intrinsics)]
#![feature(stmt_expr_attributes)]
#![feature(atomic_min_max)]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate bitflags;
extern crate sgx_types;
#[cfg(not(target_env = "sgx"))]
#[macro_use]
extern crate sgx_tstd as std;
extern crate sgx_tcrypto;
extern crate sgx_trts;
extern crate sgx_tse;
extern crate xmas_elf;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate rcore_fs;
extern crate rcore_fs_mountfs;
extern crate rcore_fs_ramfs;
extern crate rcore_fs_sefs;
#[macro_use]
extern crate derive_builder;
extern crate serde;
extern crate serde_json;

use sgx_trts::libc;
use sgx_types::*;
use std::backtrace::{self, PrintFormat};
use std::ffi::CStr; // a borrowed C string
use std::panic;

use error::*;
use prelude::*;

// Override prelude::Result with error::Result
use error::Result;

#[macro_use]
mod prelude;
#[macro_use]
mod error;

mod config;
mod entry;
mod exception;
mod fs;
mod misc;
mod net;
mod process;
mod syscall;
mod time;
mod util;
mod vm;

// Export system calls
pub use syscall::*;
// Export ECalls
pub use entry::*;
