## Sample code for Occlum Remote Attestation to generate Microsoft Azure Attestation json file

### References
* Part of the sample code, specifically the part to generate MAA format json file, is derived from the [Sample code for Intel® SGX Attestation using Microsoft Azure Attestation service and Intel® SGX SDK for Linux OS](https://github.com/Azure-Samples/microsoft-azure-attestation/tree/master/intel.sdk.attest.sample)

### Overview

The full Microsoft Azure Attestation flow includes generating a quote in an SGX enclave and then get it validated by the Microsoft [`Azure Attestation (MAA) service`](https://github.com/Azure-Samples/microsoft-azure-attestation).

There are five steps for a full flow MAA.

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
With the generated MAA format json files, users could continue on step 4 and 5 with general MAA service APIs to do validation.
