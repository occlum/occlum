# A Demo for Occlum's Embedded Mode

## Background

There are two main approaches to building SGX applications: the SDK-based 
approach (e.g., using Intel SGX SDK) and the LibOS-based approach (e.g., using 
Occlum). The SDK-based approach usually requires developers to build an SGX 
application by partitioning it into trusted and untrusted halves, while the 
LibOS-based approach runs the entire application inside an enclave.

Both approaches have their pros and cons. The SDK-based approach lets the 
developers decide which components are to be or not to be put into enclaves. 
Thus, it provides the flexibility and customizability that is attractive to 
advanced developers. However, this requires non-trivial efforts from the 
developers, especially when porting existing applications or libraries into 
enclaves. Furthermore, whichever the SDK being used, the developers are 
typically bound to a specific programming language and only provided with a 
subset of the functionality or features that are supported by the programming 
language and its libraries.

In contrast, the LibOS-based approach offers binary-level or code-level 
compatibility so that a legacy application or library can be ported into an 
enclave with minimal effort. But as the whole application is hosted by the 
LibOS inside the enclave, the developers are given no mechanism to efficiently 
offloading some functionalities of the application outside the enclave.

## The Embedded Mode

The embedded mode of Occlum brings the advantages of the SDK-based approach to 
the LibOS-based approach. As the name suggests, this mode enables developers to 
embed Occlum in an SGX application: link Occlum as a shared library to the SGX 
application and use Occlum's APIs to load and execute trusted programs in an 
enclave. This gives the developers both a complete control over the untrusted 
components outside the enclave and a Linux-compatible environment for the 
trusted programs inside the enclave. The trusted programs and the untrusted 
components can communicate with each other efficiently via shared (untrusted) 
memory. In short, the embedded mode combines the best of the two approaches.

## This Demo

To demonstrate the usage and the advantage of the embedded mode, we provide a 
benchmark program that measures the cross-enclave memory throughput, where data 
is `memcpy`'ed from an untrusted component outside the enclave to a trusted 
program inside the enclave.

The trusted program is under `trusted_memcpy_bench/`. Running upon Occlum, this 
program is given an untrusted buffer outside the enclave and measures the 
memory throughput achieved by repeatedly `memcpy`ing.

The untrusted component is under `bench_driver/`, which is a normal Linux 
program except that is linked with the Occlum PAL library and uses Occlum PAL 
APIs to load and execute `trusted_memcpy_bench` program. The untrusted buffer 
required by `trusted_memcpy_bench` is prepared by `bench_driver`.

## How to Build and Run

To build the two components, use the following command
```
make
```

To run the benchmark, use the following command
```
make test
```

To test in SGX simulation mode, use the following command
```
SGX_MODE=SIM make
SGX_MODE=SIM make test
```