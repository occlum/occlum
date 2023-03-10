![Occlum logo](docs/images/logo.png)
## <!-- render a nicely looking grey line to separate the logo from the content  -->
[![All Contributors](https://img.shields.io/badge/all_contributors-7-orange.svg?style=flat-square)](CONTRIBUTORS.md)
[![Essential Test](https://github.com/occlum/occlum/workflows/Essential%20Test/badge.svg?branch=master)](https://github.com/occlum/occlum/actions?query=workflow%3A%22Essential+Test%22)
[![SGX Hardware Mode Test](https://github.com/occlum/occlum/workflows/SGX%20Hardware%20Mode%20Test/badge.svg?branch=master)](https://github.com/occlum/occlum/actions?query=workflow%3A%22SGX+Hardware+Mode+Test%22)
[![Demo Test](https://github.com/occlum/occlum/workflows/Demo%20Test/badge.svg?branch=master)](https://github.com/occlum/occlum/actions?query=workflow%3A%22Demo+Test%22)

**NEWS:** Our paper _Occlum: Secure and Efficient Multitasking Inside a Single Enclave of Intel SGX_ has been accepted by [ASPLOS'20](https://asplos-conference.org/programs/). This research paper highlights the advantages of the single-address-space architecture adopted by Occlum and describes a novel in-enclave isolation mechanism that complements this approach. The paper can be found on [ACM Digital Library](https://dl.acm.org/doi/abs/10.1145/3373376.3378469) and [Arxiv](https://arxiv.org/abs/2001.07450).

Occlum is a *memory-safe*, *multi-process* library OS (LibOS) for [Intel SGX](https://software.intel.com/en-us/sgx). As a LibOS, it enables *legacy* applications to run on SGX with *little or even no modifications* of source code, thus protecting the confidentiality and integrity of user workloads transparently.

Occlum has the following salient features:

  * **Efficient multitasking.** Occlum offers _light-weight_ LibOS processes: they are light-weight in the sense that all LibOS processes share the same SGX enclave. Compared to the heavy-weight, per-enclave LibOS processes, Occlum's light-weight LibOS processes is up to _1,000X faster_ on startup and _3X faster_ on IPC. In addition, Occlum offers an optional [**PKU**](./docs/pku_manual.md) (Protection Keys for Userspace) feature to isolate the Occlum userspace processes if needed.
  * **Multiple file system support.** Occlum supports various types of file systems, e.g., _read-only hashed FS_ (for integrity protection), _writable encrypted FS_ (for confidentiality protection), _untrusted host FS_ (for convenient data exchange between the LibOS and the host OS).
  * **Memory safety.** Occlum is the _first_ SGX LibOS written in a memory-safe programming language ([Rust](https://www.rust-lang.org/)). Thus, Occlum is much less likely to contain low-level, memory-safety bugs and is more trustworthy to host security-critical applications.
  * **Ease-of-use.** Occlum provides user-friendly build and command-line tools. Running applications on Occlum inside SGX enclaves can be as simple as only typing several shell commands (see the next section).

## Occlum Documentation

The official Occlum documentation can be found at [`https://occlum.readthedocs.io`](https://occlum.readthedocs.io).

Some quick links are as below.

* [`Quick Start`](https://occlum.readthedocs.io/en/latest/quickstart.html#)
* [`Build and Install`](https://occlum.readthedocs.io/en/latest/build_and_install.html#)
* [`Occlum Configuration`](https://occlum.readthedocs.io/en/latest/occlum_configuration.html)
* [`Occlum Compatible Executable Binaries`](https://occlum.readthedocs.io/en/latest/binaries_compatibility.html)
* [`Demos`](https://occlum.readthedocs.io/en/latest/Demos/demos.html)
* [`Q & A`](https://occlum.readthedocs.io/en/latest/qa.html)

## What is the Implementation Status?

Occlum is being actively developed. We now focus on implementing more system calls and additional features required in the production environment, including baremetal server and public cloud (Aliyun, Azure, ...) VM.

## How about the Internal Working?

The high-level architecture of Occlum is summarized in the figure below:

![Arch Overview](docs/images/arch_overview.png)

## Why the Name?

The project name Occlum stems from the word *Occlumency* coined in Harry Potter series by J. K. Rowling. In *Harry Potter and the Order of Phoenix*, Occlumency is described as:

> The magical defence of the mind against external penetration. An obscure branch of magic, but a highly useful one... Used properly, the power of Occlumency will help shield you from access or influence.

The same thing can be said for Occlum, not for the mind, but for the program:

> The magical defence of the program against external penetration. An obscure branch of technology, but a highly useful one... Used properly, the power of Occlum will help shield your program from access or influence.

Of course, Occlum must be run on Intel x86 CPUs with SGX support to do its magic.

## Contributors

Contributions of any kind are welcome! We will publish contributing guidelines and accept pull requests after the project gets more stable.

Thanks go to [all these wonderful contributors to this project](CONTRIBUTORS.md).

## License

Occlum is released under BSD License. See the copyright information [here](LICENSE).
