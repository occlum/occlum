# Occlum Local Attestation Demo

This project demonstrates how an SGX SDK enclave can do local attestation with and establish a secure connection to an Occlum enclave, where a trusted app running upon the Occlum LibOS.

## Introduction

This project consists of four components:

1. **EnclaveInitiator**: an SGX SDK enclave that initiates requests for local attestation;
2. **AppInitiator**: an untrusted app that hosts the enclave of EnclaveInitiator and sends the enclave's requests to a responder enclave through socket.
3. **AppResponder**: a trusted socket server hosted by Occlum LibOS in another enclave, which responses to the requests initiated by EnclaveInitator and sent by AppInitiator;
4. **DiffieHellmanLibrary**: a library used by AppResponder that implements an Attestation-based, Diffie-Hellman Key Exchange protocol.

## Interopability

To do local attestation and establish a secure channel between two enclaves, they must talk in the same protocol. The protocol chosen and implemented by Intel SGX SDK is an Attestation-based, Diffie-Hellman Key Exchange protocol, whose APIs are defined in `sgx_dh.h` and used by the local attestation demo shipped with Intel SGX SDK.

To achieve interoperability with SGX SDK-based enclaves, an Occlum enclave must also talk in this protocol. But apps hosted by Occlum LibOS do not and may not use Intel SGX SDK (at least directly), thus no access to the protocol APIs exposed in `sgx_dh.h`. To resolve this problem, we build DiffieHellmanLibrary, which provides the same set of APIs as `sgx_dh.h`. And to ensure the full compatibility with Intel SGX SDK, DiffieHellmanLibrary actually reuses the code of Intel SGX SDK as much as possible. This is why EnclaveInitiator and AppResponder can talk to and negotiate with each other.

## Local Attestation through Ioctls

In the same vein as remote attestation, Occlum LibOS itself only provides three minimal APIs regarding local attestation and leaves most of the work to the user space. The three APIs are exposed to the user space through ioctls on device `/dev/sgx`. We offer a user-space library (i.e., DiffieHellmanLibrary) to hide the low-level details of ioctls and provide a high-level, user-friendly APIs. This design achieves both the easy-of-use of high-level APIs and the flexibility of low-level APIs.

## How to Build and Run

**Step 1.** Get prerequisites
```shell
./download_src_and_build_deps.sh
```
which downloads some source code and build dependencies (e.g., OpenSSL).

**Step 2.** Build and run the demo
```shell
make
make test
```

To run the demo in the SGX simulation mode, do `export SGX_MODE=SIM` before running the commands above.
