## read_write_sample for SGX
This is an example of using async-file in SGX. 
This example combines read_write_sample of async-file and hello-rust example of incubator-teaclave-sgx-sdk.
- ./app : untrusted code
- ./bin : executable program
- ./enclave : trusted code
- ./lib : library

### run read_write_sample in SGX
1. ```make```
2. ```cd bin && ./app```
