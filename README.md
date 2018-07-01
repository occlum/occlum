# Rusgx

Rusgx is a single-address-space library OS for Intel SGX. It is written in Rust programming language for memory safety. This project is still work-in-progress.

## How to build

### Prerequisite

Rusgx depends on [Rust SGX SDK](https://github.com/baidu/rust-sgx-sdk/). So, make sure Rust SGX SDk can be built properly. We have tested with Rust SGX SDK 1.0, Rust 1.26.0 and Intel SGX SDK 2.1 on Ubuntu 16.04.

### Compile

The first time to compile the project, there are some dependencies must be first downloaded. To do this, run the following commands

    cd path/to/rusgx
    make init

Then, compile the project with the following commands

    cd src/
    make

## How to use

The long-term goal is to integrate a dynamic loader into the library OS so that a single instance of Rusgx can run unmodified executables in multiple software-isolated processes. For now, we don't have to a dynamic loader; so, the temporary solution is to statically link the library OS and the executable together. Another related but different issue is the lack of the fully-fleged C standard library. We now use SGX SDK's tlibc, as it does not depend on syscalls and hence is easy to integrate. We plan to use musl libc in the future.

### Run all tests

Build and run all tests with the following commands

    cd test/
    make
    make test

### Write new program or test

Add a C file, say `try.c`, in `/test`. To compile the program, run

    make try

To run the program, run

    make test-try

    
