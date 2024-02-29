// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License..

#![crate_name = "helloworldsampleenclave"]
#![crate_type = "staticlib"]
#![cfg_attr(not(target_env = "sgx"), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]

extern crate sgx_trts;
extern crate sgx_types;
#[cfg(not(target_env = "sgx"))]
#[macro_use]
extern crate sgx_tstd as std;
extern crate sgx_libc as libc;

extern crate vdso_time;
extern crate lazy_static;

use sgx_types::*;
use std::prelude::v1::*;

include!("../../../common/example.rs");
include!("../../../common/bench.rs");

#[no_mangle]
pub extern "C" fn run_sgx_example() -> sgx_status_t {
    // std::backtrace::enable_backtrace("enclave.signed.so", std::backtrace::PrintFormat::Full);
    println!("[ECALL] run_sgx_example");

    example();

    vdso_benchmarks();

    sgx_status_t::SGX_SUCCESS
}
