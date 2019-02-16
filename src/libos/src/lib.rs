#![crate_name = "rusgx"]
#![crate_type = "staticlib"]

#![cfg_attr(not(target_env = "sgx"), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]

extern crate sgx_types;
#[cfg(not(target_env = "sgx"))]
#[macro_use]
extern crate sgx_tstd as std;
use sgx_types::*;

extern "C" {
    pub fn main() -> c_int;
}

#[no_mangle]
pub extern "C" fn libos_boot() -> sgx_status_t {
    println!("{}", "LibOS boots");
    unsafe { main(); }
    sgx_status_t::SGX_SUCCESS
}

pub mod syscall;
