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

extern crate sgx_io_uring_ocalls;
extern crate sgx_types;
extern crate sgx_urts;
use sgx_types::*;
use sgx_urts::SgxEnclave;

pub use sgx_io_uring_ocalls::*;

static ENCLAVE_FILE: &'static str = "enclave.signed.so";

extern "C" {
    fn run_sgx_bench(
        eid: sgx_enclave_id_t,
        retval: *mut sgx_status_t,
        file_num: usize,
        file_block_size: usize,
        file_total_size: usize,
        is_read: bool,
        is_seq: bool,
        use_fsync: bool,
        use_direct: bool,
        loops: usize,
    ) -> sgx_status_t;
}

fn init_enclave() -> SgxResult<SgxEnclave> {
    let mut launch_token: sgx_launch_token_t = [0; 1024];
    let mut launch_token_updated: i32 = 0;
    // call sgx_create_enclave to initialize an enclave instance
    // Debug Support: set 2nd parameter to 1
    let debug = 1;
    let mut misc_attr = sgx_misc_attribute_t {
        secs_attr: sgx_attributes_t { flags: 0, xfrm: 0 },
        misc_select: 0,
    };
    SgxEnclave::create(
        ENCLAVE_FILE,
        debug,
        &mut launch_token,
        &mut launch_token_updated,
        &mut misc_attr,
    )
}

fn main() {
    use std::env;
    let args: Vec<String> = env::args().collect();
    let kb_size = 1024;
    let mb_size = kb_size * kb_size;
    let mut file_num: usize = 1;
    let mut file_block_size: usize = 4 * kb_size;
    let mut file_total_size: usize = 100 * mb_size;
    let mut is_read: bool = true;
    let mut is_seq: bool = true;
    let mut use_fsync: bool = false;
    let mut use_direct: bool = false;
    let mut loops: usize = 100;
    if args.len() > 8 {
        file_num = args[1].parse().unwrap();
        file_block_size = args[2].parse::<usize>().unwrap() * kb_size;
        file_total_size = args[3].parse::<usize>().unwrap() * mb_size;
        is_read = args[4].parse().unwrap();
        is_seq = args[5].parse().unwrap();
        use_fsync = args[6].parse().unwrap();
        use_direct = args[7].parse().unwrap();
        loops = args[8].parse().unwrap();
    }

    let enclave = match init_enclave() {
        Ok(r) => r,
        Err(x) => {
            println!("[-] Init Enclave Failed {}!", x.as_str());
            return;
        }
    };

    let mut retval = sgx_status_t::SGX_SUCCESS;
    let result = unsafe {
        run_sgx_bench(
            enclave.geteid(),
            &mut retval,
            file_num,
            file_block_size,
            file_total_size,
            is_read,
            is_seq,
            use_fsync,
            use_direct,
            loops,
        )
    };
    match result {
        sgx_status_t::SGX_SUCCESS => {}
        _ => {
            println!("[-] ECALL Enclave Failed {}!", result.as_str());
            return;
        }
    }
    match retval {
        sgx_status_t::SGX_SUCCESS => {}
        _ => {
            println!("[-] ECALL Returned Error {}!", retval.as_str());
            return;
        }
    }
    // enclave.destroy();
    std::process::exit(1);
}
