# Occlum

Occlum is a memory-safe, multi-process library OS for Intel SGX. As a library OS, it enables *unmodified* applications to run on SGX, thus protecting the confidentiality and integrity of user workloads transparently. 

Compared to existing library OSes for SGX, Occlum has following unprecedented features:

  * **Memory safety.** The library OS itself is written in Rust, a memory-safe programming language, thus free from low-level, memory bugs;
  * **Efficient multitasking.** The library OS has a complete and efficient multi-process support, including fast process creation, low-cost IPC, code sharing (e.g., shared libraries) and data sharing (e.g., encrypted file systems).
  * **Fault isolation** - The crash of one user process cannot crash the library OS or other user processes, which is good for system stability and data integrity.

## How to Build?

### Dependencies

Occlum LibOS has several *explicit* and *implicit* dependencies: the former ones must be installed manually, while the latter ones are downloaded and compiled automatically via Makefile.

Explicit dependencies are listed below:

   1. [Occlum's fork of Intel SGX SDK](https://github.com/occlum/linux-sgx/tree/for_occlum). See [README.md](https://github.com/occlum/linux-sgx/blob/for_occlum/README.md) for how to compile and install.
   1. [Occlum's fork of LLVM toolchain](https://github.com/occlum/llvm/tree/for_occlum). See [README.occlum.md](https://github.com/occlum/llvm/blob/for_occlum/README.occlum.md) for how to compile and install.
   1. [Occlum's fork of musl libc](https://github.com/occlum/musl/tree/for_occlum). See [INSTALL](https://github.com/occlum/musl/blob/for_occlum/INSTALL) for how to compile and install.
   1. [enable_rdfsbase kernel module](https://github.com/occlum/enable_rdfsbase), which enables rdfsbase instruction and its friends. See [README.md](https://github.com/occlum/enable_rdfsbase/blob/master/README.md) for how to compile and install.
   1. [Rust programming language](https://www.rust-lang.org/). We have tested with Rust nightly-2018-10-01. Other versions of Rust may or may not work.

Implicit dependencies are managed by Git with [.gitmodules](https://github.com/occlum/libos/blob/master/.gitmodules) and compiled with Makefile. The most important implicit dependency is [Rust SGX SDK](https://github.com/baidu/rust-sgx-sdk). After downloading Occlum LibOS project, run the following command to set up the implicit dependecies:

    cd path/to/occlum/libos
    make submodule

### Compile

Then, compile the project and run tests with the following commands

    cd path/to/occlum/libos
    make
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

