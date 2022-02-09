# Occlum Remote Attestation

Occlum provides wrapped library `libocclum_dcap` for `DCAP` remote attestion applications.
This Occlum DCAP library is prebuilt as part of the Occlum toolchains in the [Occlum Docker images](https://hub.docker.com/r/occlum/occlum).

The libraries are in the path `/opt/occlum/toolchains/dcap_lib`.
```
.
|-- glibc
|   |-- dcap_test
|   |-- libocclum_dcap.a
|   `-- libocclum_dcap.so
|-- inc
|   `-- occlum_dcap.h
`-- musl
    |-- dcap_test
    |-- libocclum_dcap.a
    `-- libocclum_dcap.so
```

Two versions (glibc and musl-libc), including static and dynamic libraries are provided to meet different scenarios. Unified header file `occlum_dcap.h` is provided as well in which defines the exported APIs for DCAP quote generation and verification.

In short, applications can link to the prebuilt `libocclum_dcap.so` and use the APIs defined in `occlum_dcap.h` for their usage.

For details how to use the library, please refer to the [`demo`](../demos/remote_attestation/dcap/).

The source code of the library is in the [`path`](../tools/toolchains/dcap_lib/).

## SGX KSS (Key Separation and Sharing feature) support

Starting from SGX2, there is a new Key Separation and Sharing feature which provides more  flexibility. The new feature gives user a chance to fill in some meaningful information to the enclave either in the signing or running stage.

* Signning stage:
```
ISVFAMILYID, 16 bytes
ISVEXTPRODID, 16 bytes
```
* Running stage:
```
CONFIG ID, 64 bytes
CONFIG SVN, 16 bits
```
Occlum can support both above by either modifying the fields in `Occlum.json` (for `Signning stage`) or using Occlum run arguments `"--config-id"` or `"--config-svn"` (for `Running stage`).

Details please refer to the [RFC](https://github.com/occlum/occlum/issues/589).


## References

- [DCAP Quick Install Guide](https://software.intel.com/content/www/us/en/develop/articles/intel-software-guard-extensions-data-center-attestation-primitives-quick-install-guide.html)

- [Intel(R) Software Guard Extensions Data Center Attestation Primitives](https://github.com/intel/SGXDataCenterAttestationPrimitives)