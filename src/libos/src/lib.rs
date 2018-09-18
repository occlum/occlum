#![allow(unused)]

#![crate_name = "libos"]
#![crate_type = "staticlib"]

#![cfg_attr(not(target_env = "sgx"), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]
#![feature(allocator_api)]
#![feature(integer_atomics)]

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

mod vma;
mod process;
mod syscall;
mod elf_helper;
mod mm;

use process::spawn_process;
use process::run_task;

/// Export system calls
pub use syscall::{rusgx_write};

#[no_mangle]
pub extern "C" fn libos_boot(path_buf: *const i8) -> i32 {
    let path_str = unsafe {
        CStr::from_ptr(path_buf).to_string_lossy().into_owned()
    };
    println!("LibOS boots: {}", path_str);

    let _ = backtrace::enable_backtrace("librusgx.signed.so", PrintFormat::Short);
    panic::catch_unwind(||{
        backtrace::__rust_begin_short_backtrace(||{
            let _ = spawn_process(&path_str);
        })
    }).ok();

    0
}

#[no_mangle]
pub extern "C" fn libos_run() -> i32 {
    let _ = backtrace::enable_backtrace("librusgx.signed.so", PrintFormat::Short);
    panic::catch_unwind(||{
        backtrace::__rust_begin_short_backtrace(||{
            let _ = run_task();
        })
    }).ok();

    0
}

