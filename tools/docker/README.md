# Building Occlum Docker images

This folder contains scripts and Dockerfiles for users to build the Docker images
for Occlum. An Occlum Docker image sets up the development environment for
Occlum and also gets Occlum preinstalled.


## How to Build

### Docker image for development

Currently, three Linux OS distributions are supported: Ubuntu 20.04, aliyunlinux3 and anolis8.8.

To build an Occlum Docker image, run the following command
```
./build_image.sh <OCCLUM_LABEL> <OS_NAME> <OCCLUM_BRANCH>
```
where `<OCCLUM_LABEL>` is an arbitrary string chosen by the user to
describe the version of Occlum preinstalled in the Docker image
(e.g., "latest", "0.24.0", and "prerelease") and `<OS_NAME>` is the
name of the OS distribution that the Docker image is based on.
Currently, `<OS_NAME>` must be one of the following values:
`ubuntu20.04`, `aliyunlinux3` and `anolis8.8`.
`<OCCLUM_BRANCH>` indicates which the docker image is built on, e.g "0.24.0".
It is optional, if not provided, "master" branch will be used.

The resulting Docker image will have `occlum/occlum:<OCCLUM_LABEL>-<OS_NAME>` as its label.

### Docker image for runtime

Currently, only one Linux OS distributions are supported for runtime docker image: Ubuntu 20.04.

The Occlum runtime docker image has the smallest size, plus supports running prebuilt Occlum instance.

To build an Occlum runtime Docker image, run the following command
```
./build_rt_image.sh <OCCLUM_VERSION> <OS_NAME> <SGX_PSW_VERSION> <SGX_DCAP_VERSION>

<OCCLUM_VERSION>:
    The Occlum version is built on, e.g "0.29.7".
    Make sure this Occlum version debian packages are available in advance.

<OS_NAME>:
    The name of the OS distribution that the Docker image is based on. Currently, <OS_NAME> must be one of the following values:
        ubuntu20.04         Use Ubuntu 20.04 as the base image

<SGX_PSW_VERSION>:
    The SGX PSW version libraries expected to be installed in the runtime docker image.

<SGX_DCAP_VERSION>:
    The SGX DCAP version libraries expected to be installed in the runtime docker image.
```

The resulting Docker image will have `occlum/occlum:<OCCLUM_VERSION>-rt-<OS_NAME>` as its label.

Just note, that the **<OCCLUM_VERSION>**, **<SGX_PSW_VERSION>** and **<SGX_DCAP_VERSION>** have dependencies. Details please refer to Dockerfile.ubuntu20.04.

For example, building Occlum runtime docker image for version 0.29.7.
```
./build_rt_image.sh 0.29.7 ubuntu20.04 2.17.100.3 1.14.100.3
```
