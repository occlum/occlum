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
mod errno;
mod fs;
mod process;
mod syscall;
mod vm;
mod util;
mod time;

use prelude::*;

/// Export system calls
pub use syscall::*;

// TODO: return meaningful exit code
#[no_mangle]
pub extern "C" fn libos_boot(path_buf: *const i8) -> i32 {
    let path_str = unsafe {
        CStr::from_ptr(path_buf).to_string_lossy().into_owned()
    };
    let _ = backtrace::enable_backtrace("libocclum.signed.so", PrintFormat::Short);
    panic::catch_unwind(||{
        backtrace::__rust_begin_short_backtrace(||{
            match do_boot(&path_str) {
                Ok(()) => 0,
                Err(err) => EXIT_STATUS_INTERNAL_ERROR,
            }
        })
    }).unwrap_or(EXIT_STATUS_INTERNAL_ERROR)
}

#[no_mangle]
pub extern "C" fn libos_run() -> i32 {
    let _ = backtrace::enable_backtrace("libocclum.signed.so", PrintFormat::Short);
    panic::catch_unwind(||{
        backtrace::__rust_begin_short_backtrace(||{
            match do_run() {
                Ok(exit_status) => exit_status,
                Err(err) => EXIT_STATUS_INTERNAL_ERROR,
            }
        })
    }).unwrap_or(EXIT_STATUS_INTERNAL_ERROR)
}

// Use 127 as a special value to indicate internal error from libos, not from
// user programs, although it is completely ok for a user program to return 127.
const EXIT_STATUS_INTERNAL_ERROR : i32 = 127;

// TODO: make sure do_boot can only be called once
fn do_boot(path_str: &str) -> Result<(), Error> {
    util::mpx_enable()?;

    let argv = std::vec::Vec::new();
    let envp = std::vec::Vec::new();
    let file_actions = Vec::new();
    let parent = &process::IDLE_PROCESS;
    process::do_spawn(&path_str, &argv, &envp, &file_actions, parent)?;

    Ok(())
}

// TODO: make sure do_run() cannot be called before do_boot()
fn do_run() -> Result<i32, Error> {
    let exit_status = process::run_task()?;
    Ok(exit_status)
}
