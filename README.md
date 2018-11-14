# Occlum

Occlum is a memory-safe, multi-process library OS for Intel SGX. As a library OS, it enables *unmodified* applications to run on SGX, thus protecting the confidentiality and integrity of user workloads transparently. 

Compared to existing library OSes for SGX, Occlum has following unprecedented features:

  * **Memory safety.** The library OS itself is written in Rust, a memory-safe programming language, thus free from low-level, memory bugs;
  * **Efficient multitasking.** The library OS has a complete and efficient multi-process support, including fast process creation, low-cost IPC, code sharing (e.g., shared libraries) and data sharing (e.g., encrypted file systems).
  * **Fault isolation** - The crash of one user process cannot crash the library OS or other user processes, which is good for system stability and data integrity.

## How to Build?

### Prerequisite

Occlum depends on [Rust SGX SDK](https://github.com/baidu/rust-sgx-sdk/) and [Intel SGX SDK](https://github.com/intel/linux-sgx/). So, make sure Rust SGX SDK and Intel SGX SDK can be built properly on your machine. We have tested with Rust SGX SDK 1.1, Rust nightly-2018-08-25 and Intel SGX SDK 2.2 on Ubuntu 16.04.

### Compile

The first time to compile the project, there are some dependencies must be first downloaded. To do this, run the following commands

    cd path/to/occlum-libos
    make submodule

Then, compile the project with the following commands

    cd src/
    make

### Run tests

Build and run all tests with the following commands

    make test

## How to Use?

To be written...

## How it Works?

To be written...

### Architecture Overview

To be written...

### Software Isolated Processes (SIPs)

To be written...

## Why the Name?

The project name Occlum stems from the word *Occlumency* coined in Harry Porter series by J. K. Rowling. In *Harry Porter and the Order of Pheonix*, Occlumency is described as:

> The magical defence of the mind against external penetration. An obscure branch of magic, but a highly useful one... Used properly, the power of Occlumency wil help sheild you from access or influence.

The same thing can be said to Occlum, not for mind, but program:

> The magical defence of the program agaist external penetration. An obsecure branch of technology, but a highly useful one... Used properly, the power of Occlum will help sheild your program from access or influence.

Of course, Occlum must be run on Intel x86 CPUs with SGX support to do its magic.

