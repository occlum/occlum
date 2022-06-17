## Sample code for doing Microsoft Azure Attestation in Occlum

This demo is programming in RUST, based on the Azure provided [`REST APIs`](https://docs.microsoft.com/en-us/rest/api/attestation/). It provides steps to do SGX quote generation and attestation.

* Build

1. Pull rust-sgx-sdk submodule which is the dependence of occlum dcap library.

```
# cd occlum
# git submodule update --init
```

2. Do the build with the [`scrit`](./build.sh).

```
# ./build.sh
```

* Run
```
# cd occlum_instance
# occlum run /bin/azure_att
```

If successful, it prints the Azure attestation token.