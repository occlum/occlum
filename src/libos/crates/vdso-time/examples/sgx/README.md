## example for SGX
This is an example of using vdso-time in SGX. 
This example combines vdso-time example of io_uring and hello-rust example of incubator-teaclave-sgx-sdk.
- ./app : untrusted code
- ./bin : executable program
- ./enclave : trusted code
- ./lib : library

### run example in SGX
1. ```make```
2. ```cd bin && ./app```
