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

extern crate io_uring;
extern crate io_uring_callback;
extern crate lazy_static;
extern crate slab;

use sgx_trts::libc;
use sgx_types::*;
use std::collections::VecDeque;
use std::os::unix::io::RawFd;
use std::prelude::v1::*;
use std::ptr;
use std::sync::SgxMutex as Mutex;

use io_uring::opcode::types;
use io_uring_callback::{Builder, Handle, IoUring};
use lazy_static::lazy_static;
use slab::Slab;

lazy_static! {
    static ref TOKEN_QUEUE: Mutex<VecDeque<(Token, i32)>> = Mutex::new(VecDeque::new());
    static ref HANDLE_SLAB: Mutex<slab::Slab<Handle>> = Mutex::new(slab::Slab::new());
}

#[derive(Clone, Debug)]
enum Token {
    Accept,
    Poll {
        fd: RawFd,
    },
    Read {
        fd: RawFd,
        buf_index: usize,
    },
    Write {
        fd: RawFd,
        buf_index: usize,
        offset: usize,
        len: usize,
    },
}

pub struct AcceptCount {
    fd: types::Fd,
    count: usize,
}

impl AcceptCount {
    fn new(fd: RawFd, count: usize) -> AcceptCount {
        AcceptCount {
            fd: types::Fd(fd),
            count: count,
        }
    }

    pub fn try_push_accept(&mut self, ring: &IoUring) {
        while self.count > 0 {
            let to_complete_token = Token::Accept;
            let mut handle_slab = HANDLE_SLAB.lock().unwrap();
            let slab_entry = handle_slab.vacant_entry();
            let slab_key = slab_entry.key();

            let complete_fn = move |retval: i32| {
                let mut queue = TOKEN_QUEUE.lock().unwrap();
                queue.push_back((to_complete_token, retval));

                HANDLE_SLAB.lock().unwrap().remove(slab_key);
            };

            let handle =
                unsafe { ring.accept(self.fd, ptr::null_mut(), ptr::null_mut(), 0, complete_fn) };

            slab_entry.insert(handle);

            self.count -= 1;
        }
    }
}

#[no_mangle]
pub extern "C" fn run_sgx_example() -> sgx_status_t {
    // std::backtrace::enable_backtrace("enclave.signed.so", std::backtrace::PrintFormat::Full);
    println!("[ECALL] run_sgx_example");

    let ring = Builder::new()
        .setup_sqpoll(Some(500/* ms */))
        .build(256)
        .unwrap();

    let socket_fd = {
        let socket_fd = unsafe { libc::ocall::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        if socket_fd < 0 {
            println!("[ECALL] create socket failed, ret: {}", socket_fd);
            return sgx_status_t::SGX_ERROR_UNEXPECTED;
        }

        let ret = unsafe {
            let servaddr = libc::sockaddr_in {
                sin_family: libc::AF_INET as u16,
                sin_port: 3456_u16.to_be(),
                sin_addr: libc::in_addr { s_addr: 0 },
                sin_zero: [0; 8],
            };
            libc::ocall::bind(
                socket_fd,
                &servaddr as *const _ as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_in>() as u32,
            )
        };
        if ret < 0 {
            println!("[ECALL] bind failed, ret: {}", ret);
            unsafe {
                libc::ocall::close(socket_fd);
            }
            return sgx_status_t::SGX_ERROR_UNEXPECTED;
        }

        let ret = unsafe { libc::ocall::listen(socket_fd, 10) };
        if ret < 0 {
            println!("[ECALL] listen failed, ret: {}", ret);
            unsafe {
                libc::ocall::close(socket_fd);
            }
            return sgx_status_t::SGX_ERROR_UNEXPECTED;
        }
        socket_fd
    };

    let mut bufpool = Vec::with_capacity(64);
    let mut buf_alloc = Slab::with_capacity(64);

    println!("[ECALL] listen 127.0.0.1:3456");

    let mut accept = AcceptCount::new(socket_fd, 3);

    loop {
        accept.try_push_accept(&ring);

        ring.trigger_callbacks();

        let mut queue = TOKEN_QUEUE.lock().unwrap();
        while !queue.is_empty() {
            let (token, ret) = queue.pop_front().unwrap();
            match token {
                Token::Accept => {
                    println!("[ECALL] accept");

                    accept.count += 1;

                    let fd = ret;

                    let to_complete_token = Token::Poll { fd };
                    let mut handle_slab = HANDLE_SLAB.lock().unwrap();
                    let slab_entry = handle_slab.vacant_entry();
                    let slab_key = slab_entry.key();

                    let complete_fn = move |retval: i32| {
                        let mut queue = TOKEN_QUEUE.lock().unwrap();
                        queue.push_back((to_complete_token, retval));

                        HANDLE_SLAB.lock().unwrap().remove(slab_key);
                    };

                    let handle =
                        unsafe { ring.poll(types::Fd(fd), libc::POLLIN as _, complete_fn) };

                    slab_entry.insert(handle);
                }
                Token::Poll { fd } => {
                    let (buf_index, buf) = match bufpool.pop() {
                        Some(buf_index) => (buf_index, &mut buf_alloc[buf_index]),
                        None => {
                            let buf = Box::new(unsafe {
                                std::slice::from_raw_parts_mut(
                                    libc::ocall::malloc(2048) as *mut u8,
                                    2048,
                                )
                            });
                            let buf_entry = buf_alloc.vacant_entry();
                            let buf_index = buf_entry.key();
                            (buf_index, buf_entry.insert(buf))
                        }
                    };

                    let to_complete_token = Token::Read { fd, buf_index };
                    let mut handle_slab = HANDLE_SLAB.lock().unwrap();
                    let slab_entry = handle_slab.vacant_entry();
                    let slab_key = slab_entry.key();

                    let complete_fn = move |retval: i32| {
                        let mut queue = TOKEN_QUEUE.lock().unwrap();
                        queue.push_back((to_complete_token, retval));

                        HANDLE_SLAB.lock().unwrap().remove(slab_key);
                    };

                    let handle = unsafe {
                        ring.read(
                            types::Fd(fd),
                            buf.as_mut_ptr(),
                            buf.len() as _,
                            0,
                            0,
                            complete_fn,
                        )
                    };

                    slab_entry.insert(handle);
                }
                Token::Read { fd, buf_index } => {
                    if ret == 0 {
                        bufpool.push(buf_index);

                        println!("[ECALL] shutdown");

                        unsafe {
                            libc::ocall::close(fd);
                        }
                    } else {
                        let len = ret as usize;
                        let buf = &buf_alloc[buf_index];

                        let to_complete_token = Token::Write {
                            fd,
                            buf_index,
                            len,
                            offset: 0,
                        };
                        let mut handle_slab = HANDLE_SLAB.lock().unwrap();
                        let slab_entry = handle_slab.vacant_entry();
                        let slab_key = slab_entry.key();

                        let complete_fn = move |retval: i32| {
                            let mut queue = TOKEN_QUEUE.lock().unwrap();
                            queue.push_back((to_complete_token, retval));

                            HANDLE_SLAB.lock().unwrap().remove(slab_key);
                        };

                        let handle = unsafe {
                            ring.write(types::Fd(fd), buf.as_ptr(), len as _, 0, 0, complete_fn)
                        };

                        slab_entry.insert(handle);
                    }
                }
                Token::Write {
                    fd,
                    buf_index,
                    offset,
                    len,
                } => {
                    let write_len = ret as usize;

                    if offset + write_len >= len {
                        bufpool.push(buf_index);

                        let to_complete_token = Token::Poll { fd };
                        let mut handle_slab = HANDLE_SLAB.lock().unwrap();
                        let slab_entry = handle_slab.vacant_entry();
                        let slab_key = slab_entry.key();

                        let complete_fn = move |retval: i32| {
                            let mut queue = TOKEN_QUEUE.lock().unwrap();
                            queue.push_back((to_complete_token, retval));

                            HANDLE_SLAB.lock().unwrap().remove(slab_key);
                        };

                        let handle =
                            unsafe { ring.poll_add(types::Fd(fd), libc::POLLIN as _, complete_fn) };

                        slab_entry.insert(handle);
                    } else {
                        let offset = offset + write_len;
                        let len = len - offset;

                        let buf = &buf_alloc[buf_index][offset..];

                        let to_complete_token = Token::Write {
                            fd,
                            buf_index,
                            offset,
                            len,
                        };
                        let mut handle_slab = HANDLE_SLAB.lock().unwrap();
                        let slab_entry = handle_slab.vacant_entry();
                        let slab_key = slab_entry.key();

                        let complete_fn = move |retval: i32| {
                            let mut queue = TOKEN_QUEUE.lock().unwrap();
                            queue.push_back((to_complete_token, retval));

                            HANDLE_SLAB.lock().unwrap().remove(slab_key);
                        };

                        let handle = unsafe {
                            ring.write(types::Fd(fd), buf.as_ptr(), len as _, 0, 0, complete_fn)
                        };

                        slab_entry.insert(handle);
                    };
                }
            }
        }
    }
}
