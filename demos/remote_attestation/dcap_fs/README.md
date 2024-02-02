# SGX DCAP Remote Attestation Demo

This project demonstrates how to get Intel SGX DCAP (Datacenter Attestation
Primitives) Quote on Occlum. Occlum provides devfs nodes **/dev/attestation_report_data** and **/dev/attestation_quote** to ease the quote generation.

## Prerequisites

- Platform: Intel SGX enabled platform with DCAP installed. Follow [DCAP
  Quick Install
  Guide](https://software.intel.com/content/www/us/en/develop/articles/intel-software-guard-extensions-data-center-attestation-primitives-quick-install-guide.html)
  for the detailed installation procedure.

- Occlum: Compile Occlum on a DCAP-installed platform by invoking `make`. The
  compilation will look for the needed DCAP libraries. The needed libraries
  include `libsgx_quote_ex, libsgx_quote_ex_sim, libsgx_dcap_tvl,
  libsgx_dcap_ql and libsgx_dcap_quoteverify`.

You can simply start a Occlum develop container to meet above two.
A valid PCCS service should be accessible in your environment. This demo is verified in Aliyun, thus `https://sgx-dcap-server.cn-shanghai.aliyuncs.com/sgx/certification/v3/` is used as the PCCS URL. For example, 

* Start the Occlum develop container
```
docker run --rm -it \
     --device /dev/sgx/enclave --device /dev/sgx/provision \
     occlum/occlum:latest-ubuntu20.04 bash
```

In the container, update the **pccs_url** in the file `/etc/sgx_default_qcnl.conf` with the valid address.

## Run this demo on Occlum

You can build and run the DCAP quote generation demo via
```
./build_and_run.sh
```

The demo does below things:
1. Write **report data** to the node **/dev/attestation_report_data**.
2. Read the quote from node **/dev/attestation_quote**.
3. Write the quote as file to the path `/host/quote`, which in default is the file `quote` in the folder `occlum_instance`.
