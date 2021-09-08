# Demo of RATS TLS server/client running in Occlum

This project demonstrates how to run RATS TLS server/client in Occlum [`RATS TLS`](https://github.com/alibaba/inclavare-containers/tree/master/rats-tls).

## Prerequisites

First make sure the DCAP demo is working fine on your platform, either host or container ENV.
Details could be refer to [`DCAP demo`](../remote_attestation/dcap/).

As the DCAP version on Occlum may not be the latest one, could be inconsistent with the DCAP version in PCCS server.
Thus the quote verify may return `SGX_QL_QV_RESULT_OUT_OF_DATE` which means unsuccess.

In this demo, one [`workaround patch`](./0001-Consider-SGX_QL_QV_RESULT_OUT_OF_DATE-as-success.patch) is applied when building the RATS TLS to avoid above failure.

## Steps

Step 1: Download and build RATS TLS.
```shell
./download_and_build_rats_tls.sh
```
When completed, the resulting samples are installed in `/usr/share/rats-tls/samples`.
The libraries are installed in `/usr/local/lib/rats-tls`.

Step 2: Build and run RATS TLS server in Occlum on background.
```shell
./occlum_build_and_run_rats_tls_server.sh
```

Step 3: Then build and run RATS TLS client in Occlum.
```shell
./occlum_build_and_run_rats_tls_client.sh
```

If success, the server SGX `mrsigner` and `mrenclave` will be printed out.
```shell
...
[INFO] Server's SGX identity:
[INFO]   . MRENCLAVE = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
[INFO]   . MRSIGNER  = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
...
```
