# Use Enclave TLS server with Occlum

This project demonstrates how to run a server with [Enclave TLS](https://github.com/alibaba/inclavare-containers/tree/master/enclave-tls).

Step 1: Download and build Enclave TLS.
```shell
./download_and_build_enclave_tls.sh
```
When completed, the resulting server can be found at `/opt/enclave-tls/bin`.

Step 2: You can run the encalve tls server on Occlum.
```shell
./run_enclave_tls_server_in_occlum.sh
```

Step 3: To check whether the enclave tls server works, run
```shell
/opt/enclave-tls/bin/enclave-tls-client
```
in another terminal.
