## Sample code for doing Microsoft Azure Attestation in Occlum

Two examples are provided for reference. All are running in Occlum Enclave environment and verified on Azure confidential VM.

### MAA format json file generation demo [`maa_json`](./maa_json)

This demo is programming in C, covering the SGX quote generation and format the quote to MAA format json file. It doesn't cover the attestation part.

### MAA attestation flow demo [`maa_attestation`](./maa_attestation)

This demo is programming in RUST, based on the Azure provided [`REST APIs`](https://docs.microsoft.com/en-us/rest/api/attestation/). It provides steps to do SGX quote generation and attestation.

## Prerequisites

### Platform

An Azure confidential VM. Users could follow the [`guide`](https://docs.microsoft.com/en-us/azure/confidential-computing/quick-create-portal) to create one.

### Container

Start the Occlum latest docker container image for the demo in Azure confidential VM. Follow the [guide](https://github.com/occlum/occlum#how-to-use) or just try below command.

```
sudo docker run --rm -it \
    --device /dev/sgx/enclave --device /dev/sgx/provision \
    --name occlum-dev occlum/occlum:0.27.3-ubuntu20.04 bash
```

### PCK caching service

The Occlum docker container image assuming the Intel PCK caching service for DCAP remote attestation in default. But Azure has an Azure DCAP library instead, details please refer to the [`link`](https://docs.microsoft.com/en-us/azure/attestation/faq#how-can-a-verifier-obtain-the-collateral-for-sgx-attestation-supported-by-azure-attestation). To support the Occlum DCAP remote attestation running in Azure, below commands need to be executed in the Occlum docker container.

* Uninstall Intel default DCAP qpl library.
```
apt purge libsgx-dcap-default-qpl
```

* Install Azure DCAP library
```
echo "deb [arch=amd64] https://packages.microsoft.com/ubuntu/20.04/prod focal main" | sudo tee /etc/apt/sources.list.d/msprod.list
wget -qO - https://packages.microsoft.com/keys/microsoft.asc | sudo apt-key add -
apt update
apt install az-dcap-client
```
