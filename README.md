# Occlum
[![All Contributors](https://img.shields.io/badge/all_contributors-7-orange.svg?style=flat-square)](CONTRIBUTORS.md)

Occlum is a *memory-safe*, *multi-process* library OS (LibOS) for [Intel SGX](https://software.intel.com/en-us/sgx). As a LibOS, it enables *unmodified* applications to run on SGX, thus protecting the confidentiality and integrity of user workloads transparently. 

Compared to existing LibOSes for SGX, Occlum has the following salient features:

  * **Efficient multitasking.** The LibOS has a complete and efficient multi-process support, including fast process creation, low-cost IPC, shared OS services (e.g., encrypted file systems).
  * **Fault isolation.** The crash of one user process cannot crash the LibOS or other user processes, which is good for security and robustness.
  * **Memory safety.** The LibOS itself is written in [Rust](https://www.rust-lang.org/), a memory-safe programming language, thus free from low-level, memory bugs;

## Why Occlum?

### Efficient Multitasking

The primary motivation of Occlum project is to achieve efficient multitasking on LibOSes for SGX.

Multitasking is an important feature for LibOSes (or any OSes in general), but difficult to implement efficiently on SGX. It is important since virtually any non-trivial application demands more than one process. And its difficulty is evident from the fact that existing LibOSes for SGX either do not support multitasking (e.g., Haven and SCONE) or fail to do so efficiently (e.g., Graphene-SGX is nearly 10,000X slower than Linux on spawning new processes).

To realize efficient multitasking, Occlum adopts a novel *multi-process-per-enclave* approach, which runs all LibOS processes and the LibOS itself inside a single enclave. Running inside a single address space, Occlum's processes enjoy the benefits of fast startup, low-cost inter-process communication (IPC) and shared system services (e.g., encrypted file systems).

### Fault Isolation

As there are no hardware isolation mechanisms available inside an enclave, Occlum emulates the traditional OS-enforced inter-process isolation and user-kernel isolation with [Software Fault Isolation (SFI)](http://www.cse.psu.edu/~gxt29/papers/sfi-final.pdf) technique. Specifically, we design a novel SFI scheme called **Multi-Domain SFI (MDSFI)** that enables Occlum to enforce process sandbox: *any LibOS process cannot compromise or crash other LibOS processes or the LibOS itself*.

### Memory Safety

Occlum also improves the memory safety of LibOS-based, SGX-protected applications. The memory safety of C/C++ programs is still an unresolved problem (e.g., Google [syzkaller project](https://github.com/google/syzkaller) found 600+ memory bugs in Linux kernel). And it is well known that memory-safe bugs are the most common class of security vulnerabilities. Compared to existing LibOSes for SGX, Occlum improves the memory safety of SGX applications in two folds:

   1. User programs are made more resilient to memory safety vulnerabilities. Thanks to MDSFI, Occlum enforces Data Execution Prevention (DEP) to prevent code injection attacks and Control Flow Integrity (CFI) to mitigate Return-Oriented Programming (ROP) attacks. 
   1. LibOS itself is memory safe. Occlum LibOS is developed in Rust programming language, a memory-safe programming language. This reduces the odds of low-level memory-safety bugs in the LibOS, thus more trustworthy to the application developers.

## How to Use?

We have built and tested Occlum on Ubuntu 16.04 with hardware SGX support. We recommend using the Occlum Docker image to set up the development environment and give it a try quickly.

To build and test Occlum with Docker container, follow the steps listed below.

Step 1-4 are to be done on the host OS:

1. Install [Intel SGX driver for Linux](https://github.com/intel/linux-sgx), which is required by Intel SGX SDK.

2. Install [enable_rdfsbase kernel module](https://github.com/occlum/enable_rdfsbase), which enables Occlum to use `rdfsbase`-family instructions in enclaves.

3. Download the latest source code of Occlum LibOS
```
cd /your/path/to/
git clone https://github.com/occlum/libos
```
4. Run the Occlum Docker container
```
docker run -it \
  --mount type=bind,source=/your/path/to/libos,target=/root/occlum/libos \
  --device /dev/isgx \
  occlum
```
Step 5-8 are to be done on the guest OS running inside the container:

5. Start the AESM service required by Intel SGX SDK
```
/opt/intel/libsgx-enclave-common/aesm/aesm_service &
```
6. (Optional) Try the sample code of Intel SGX SDK
```
cd /opt/intel/sgxsdk/SampleCode/SampleEnclave && make && ./app
```
7. Prepare the submodules required by Occlum LiboS
```
cd /root/occlum/libos && make submodule
```
8. Compile and test Occlum LibOS
```
cd /root/occlum/libos && make && make test
```
The Occlum Dockerfile can be found at [here](tools/docker/Dockerfile). Use it to build the container directly or read it to see the dependencies of Occlum LibOS.

## What is the Implementation Status?

The current version is **only for technical preview, not ready for production use**. Yet, even with this early version, we are able to port real-world, multi-process applications such as [Fish shell](https://fishshell.com/), [GCC](https://gcc.gnu.org/), and [Lighttpd](http://www.lighttpd.net/) to SGX in less 100 LoC modifications. Thanks to the efficient multitasking support, Occlum \emph{significantly} outperforms traditional SGX LibOSes on workloads that involve process spawning.

This project is being actively developed. We now focus on implementing more system calls and additional features required in the production environment.

## Why the Name?

The project name Occlum stems from the word *Occlumency* coined in Harry Porter series by J. K. Rowling. In *Harry Porter and the Order of Pheonix*, Occlumency is described as:

> The magical defence of the mind against external penetration. An obscure branch of magic, but a highly useful one... Used properly, the power of Occlumency wil help sheild you from access or influence.

The same thing can be said to Occlum, not for mind, but program:

> The magical defence of the program agaist external penetration. An obsecure branch of technology, but a highly useful one... Used properly, the power of Occlum will help sheild your program from access or influence.

Of course, Occlum must be run on Intel x86 CPUs with SGX support to do its magic.

## Contributors

The creators of Occlum project are
  * Hongliang Tian and Shoumeng Yan from Intel Corporation (now work for Ant Financial); and
  * Youren Shen, Yu Chen, and Kang Chen from Tsinghua University.

This project follows the [all-contributors](https://allcontributors.org) specification. Contributions of any kind are welcome! We will publish contributing guidelines and accept pull requests after the project gets more stable.

Thanks go to [all these wonderful contributors for this project](CONTRIBUTORS.md).

## Disclaimer

While Occlum was originally designed by and incubated inside Intel, it is NOT an official Intel product.
