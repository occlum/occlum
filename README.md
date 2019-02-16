# Occlum
[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors)

Occlum is a *memory-safe*, *multi-process* library OS (LibOS) for [Intel SGX](https://software.intel.com/en-us/sgx). As a LibOS, it enables *unmodified* applications to run on SGX, thus protecting the confidentiality and integrity of user workloads transparently. 

Compared to existing LibOSes for SGX, Occlum has following salient features:

  * **Efficient multitasking.** The LibOS has a complete and efficient multi-process support, including fast process creation, low-cost IPC, code sharing (e.g., shared libraries) and data sharing (e.g., encrypted file systems).
  * **Fault isolation.** The crash of one user process cannot crash the LibOS or other user processes, which is good for security and robustness.
  * **Memory safety.** The LibOS itself is written in [Rust](https://www.rust-lang.org/), a memory-safe programming language, thus free from low-level, memory bugs;
  
## Why Occlum?

### Efficient Multitasking

The primary motivation of Occlum project is to achieve efficient multitasking on LibOSes for SGX.

Multitasking is an important feature for LibOSes (or any OSes in general), but difficult to implement efficiently on SGX. It is important since virtually any non-trivial application demands more than one process. And its difficulty is evident from the fact that existing LibOSes for SGX either do not support multitasking (e.g., Haven and SCONE) or fail to do so efficiently (e.g., Graphene-SGX is nearly 10,000X slower than Linux on spawning new processes).

To realize efficient multitasking, Occlum adopts a novel *multi-process-per-enclave* approach, which runs all LibOS processes and the LibOS itself inside a single enclave. Running inside a single address space, Occlum’s processes enjoy the benefits of fast startup, low-cost inter-process communication (IPC) and shared system services (e.g., encrypted file systems).

### Fault Isolation

As there are no hardware isolation mechanisms available inside an enclave, Occlum emulates the traditional OS-enforced inter-process isolation and user-kernel isolation with [Software Fault Isolation (SFI)]() technique. Specifically, we design a novel SFI scheme called **Multi-Domain SFI (MDSFI)** that enables Occlum to enforce process sandbox: *any LibOS process cannot compromise or crash other LibOS processes or the LibOS itself*.

### Memory Safety

Occlum also improves the memory safety of LibOS-based, SGX-protected applications. The memory safety of C/C++ programs is still an unresolved problem (e.g., Google [syzkaller project](https://github.com/google/syzkaller) found 600+ memory bugs in Linux kernel). And it is well known that memory-safe bugs are the most common class of security vulnerabilities. Compared to existing LibOSes for SGX, Occlum improves the memory safety of SGX applications in two folds:

   1. User programs are made more resilient to memory safety vulnerabilities. Thanks to MDSFI, Occlum enforces Data Execution Prevention (DEP) to prevent code injection attacks and Control Flow Integrity (CFI) to mitigate Return-Oriented Programming (ROP) attacks. 
   1. LibOS itself is memory safe. Occlum LibOS is developed in Rust programming language, a memory-safe programming language. This reduces the odds of low-level memory-safety bugs in the LibOS, thus more trustworthy to the application developers.

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

## What is the Implementation Status?

The current version is only for technical preview, not ready for production use. Yet, even with this early version, we can achieve a speedup of multitasking-related operations by up to three orders of magnitude, thus demonstrating the effectiveness of our multi-process-per-enclave approach.

This project is being actively developed. We now focus on implementing more system calls and hopefully enable real-world applications on Occlum soon.

## Why the Name?

The project name Occlum stems from the word *Occlumency* coined in Harry Porter series by J. K. Rowling. In *Harry Porter and the Order of Pheonix*, Occlumency is described as:

> The magical defence of the mind against external penetration. An obscure branch of magic, but a highly useful one... Used properly, the power of Occlumency wil help sheild you from access or influence.

The same thing can be said to Occlum, not for mind, but program:

> The magical defence of the program agaist external penetration. An obsecure branch of technology, but a highly useful one... Used properly, the power of Occlum will help sheild your program from access or influence.

Of course, Occlum must be run on Intel x86 CPUs with SGX support to do its magic.

## Disclaimer

While Occlum was originally designed by and incubated inside Intel, it is NOT an official Intel product.

The original source code was released by Intel under a project named SGX Multi-Process Library Operating System (SGXMPLOS). As the vendor-neutral, community-driven successor of SGXMPLOS, Occlum project is where all future development happens.

## Contributors

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore -->
| [<img src="https://avatars0.githubusercontent.com/u/568208?v=4" width="100px;" alt="Tate Tian"/><br /><sub><b>Tate Tian</b></sub>](https://github.com/tatetian)<br />[💻](https://github.com/occlum/libos/commits?author=tatetian "Code") [⚠️](https://github.com/occlum/libos/commits?author=tatetian "Tests") [📖](https://github.com/occlum/libos/commits?author=tatetian "Documentation") [🚧](#maintenance-tatetian "Maintenance") |
| :---: |
<!-- ALL-CONTRIBUTORS-LIST:END -->
The original authors of Occlum (or SGXMPLOS) are
  * Hongliang Tian and Shoumeng Yan from Intel; and
  * Youren Shen, Yu Chen, and Kang Chen from Tsinghua University.

For the names of all contributors, see [this list](CONTRIBUTOR.md).
