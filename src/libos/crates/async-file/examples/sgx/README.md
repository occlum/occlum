## seq_read_write example for SGX
This is an example of using async-file in SGX. 
This example combines seq_read_write example of async-file and hello-rust example of incubator-teaclave-sgx-sdk.
- ./app : untrusted code
- ./bin : executable program
- ./enclave : trusted code
- ./lib : library

### run seq_read_write example in SGX
1. Prepare environments.
    - clone incubator-teaclave-sgx-sdk repo to ```../../../third_parties/```. And checkout incubator-teaclave-sgx-sdk to ```d94996``` commit.
    - prepare environments for incubator-teaclave-sgx-sdk.
2. ```make```
3. ```cd bin && ./app```
