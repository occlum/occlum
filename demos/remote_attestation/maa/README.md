## Sample code for Occlum Remote Attestation to generate Microsoft Azure Attestation json file

### References
* Part of the sample code, specifically the part to generate MAA format json file, is derived from the [Sample code for Intel® SGX Attestation using Microsoft Azure Attestation service and Intel® SGX SDK for Linux OS](https://github.com/Azure-Samples/microsoft-azure-attestation/tree/master/intel.sdk.attest.sample)

## Prerequisites

- Platform: Intel SGX enabled platform with DCAP installed. Follow [DCAP
  Quick Install
  Guide](https://software.intel.com/content/www/us/en/develop/articles/intel-software-guard-extensions-data-center-attestation-primitives-quick-install-guide.html)
  for the detailed installation procedure.

- Container: Start the Occlum latest docker container image for the demo. Follow
  the [guide](https://github.com/occlum/occlum#how-to-use).

Remember to configure `/etc/sgx_default_qcnl.conf`
in the container according to your PCCS setting after running the docker image.

### Overview

The full Microsoft Azure Attestation flow includes generating a quote in an SGX enclave and then get it validated by the Microsoft Azure Attestation (MAA) service.

1. Build an SGX enclave
2. Launch an SGX enclave and get SGX quote
3. Persist SGX quote and Enclave Held Data (EHD) to JSON file
4. Call Azure Attestation for validation
5. Output validation results

This demo only covers the first three steps.

* Build and Run
```
# ./run.sh
```

Once successful, four different MAA format json files are saved in `out` dir.
