# Building Occlum Docker images

This folder contains scripts and Dockerfiles for users to build the Docker images
for Occlum. An Occlum Docker image sets up the development environment for
Occlum and also gets Occlum preinstalled.

Currently, three Linux OS distributions are supported: Ubuntu 18.04, CentOS 8.2 and aliyunlinux3.

## How to Build

To build an Occlum Docker image, run the following command
```
./build_image.sh <OCCLUM_LABEL> <OS_NAME> <OCCLUM_BRANCH>
```
where `<OCCLUM_LABEL>` is an arbitrary string chosen by the user to
describe the version of Occlum preinstalled in the Docker image
(e.g., "latest", "0.24.0", and "prerelease") and `<OS_NAME>` is the
name of the OS distribution that the Docker image is based on.
Currently, `<OS_NAME>` must be one of the following values:
`ubuntu18.04`, `centos8.2` and `aliyunlinux3`.
`<OCCLUM_BRANCH>` indicates which the docker image is built on, e.g "0.24.0".
It is optional, if not provided, "master" branch will be used.

The resulting Docker image will have `occlum/occlum:<OCCLUM_LABEL>-<OS_NAME>` as its label.
