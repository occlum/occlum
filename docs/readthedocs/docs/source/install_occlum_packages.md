# Install Occlum with Popular Package Managers

Occlum can be easily installed with popular package managers like [APT](https://en.wikipedia.org/wiki/APT_(software)) and [RPM](https://en.wikipedia.org/wiki/RPM_Package_Manager). This document walks you through the steps to install Occlum on popular Linux distributions like Ubuntu and CentOS using package managers.

## Prerequisite

**Install enable_RDFSBASE Kernel Module**

If the Kernel version is before `v5.9`, please follow [this README](https://github.com/occlum/enable_rdfsbase/blob/master/README.md) to install `enable_rdfsbase` kernel module.

## Install Occlum with APT on Ubuntu 20.04

1. Install Prerequisite
```
apt update
DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends ca-certificates gnupg2 jq make gdb wget libfuse-dev libtool tzdata rsync
```

2. Install Intel® SGX Driver and Intel® SGX PSW

Please follow [Intel SGX Installation Guide](https://download.01.org/intel-sgx/sgx-linux/2.13/docs/Intel_SGX_Installation_Guide_Linux_2.13_Open_Source.pdf) to install SGX driver and SGX PSW. SGX SDK is not required. Using PSW installer is recommanded.

To install PSW, follow the guide to add Intel® SGX repository to APT source. And then run:
```
apt-get update
apt-get install -y libsgx-dcap-ql libsgx-epid libsgx-urts libsgx-quote-ex libsgx-uae-service libsgx-dcap-quote-verify-dev
```

After installing PSW, please make sure `aesm` service is in `active (running)` state by checking:
```
service aesmd status
```

3. Install Occlum
```
echo 'deb [arch=amd64] https://occlum.io/occlum-package-repos/debian bionic main' | tee /etc/apt/sources.list.d/occlum.list
wget -qO - https://occlum.io/occlum-package-repos/debian/public.key | apt-key add -
apt-get update
apt-get install -y occlum
```

### Occlum toolchains packages

Besides, users can choose to install the toolchain installer based on the application's language. Currently, Occlum supports only `musl-gcc`, `glibc`. Users can install each one on demand.

```
apt install -y occlum-toolchains-gcc
apt install -y occlum-toolchains-glibc
```

### Occlum Runtime package

If users only expect to run the Occlum instance image, then `occlum-runtime` package is better choice for size reason.
```
apt install -y occlum-runtime
```


## Version Compatability Matrix

When version is not specified, Occlum with the latest version will be installed. If a user would like to evaluate an older version, please make sure the corresponding Intel® SGX PSW is installed.

The matrix below shows the version compatability since Occlum `0.16.0`. Please check before installing or upgrading.

| Occlum Version  |  SGX PSW Version  | Tested under Ubuntu |
| --------------- | ----------------- | ------------------- |
|     0.16.0      |       2.11        |        18.04        |
|     0.17.0      |       2.11        |        18.04        |
|     0.18.1      |       2.11        |        18.04        |
|     0.19.1      |       2.11        |        18.04        |
|     0.20.0      |       2.11        |        18.04        |
|     0.21.0      |       2.13        |        18.04        |
|     0.23.1      |       2.13.3      |        18.04        |
|     0.24.1      |       2.14        |        18.04        |
|     0.27.3      |       2.15.1      |        20.04        |
|     0.28.1      |       2.16        |        20.04        |
|     0.29.0      |       2.17.1      |        20.04        |

For more information about the packages, please checkout [here](../tools/installer/README.md).
