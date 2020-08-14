# Building Occlum Docker images

This folder contains scripts and Dockerfiles for users to build the Docker images
for Occlum. An Occlum Docker image sets up the development environment for
Occlum and also gets Occlum preinstalled.

Currently, three Linux OS distributions are supported: Ubuntu 18.04, CentOS 7.5 and CentOS 8.1.

## How to Build

To build an Occlum Docker image, run the following command
```
./build_image.sh <OCCLUM_LABEL> <OS_NAME>
```
where `<OCCLUM_LABEL>` is an arbitrary string chosen by the user to
describe the version of Occlum preinstalled in the Docker image
(e.g., "latest", "0.15.0", and "prerelease") and `<OS_NAME>` is the
name of the OS distribution that the Docker image is based on.
Currently, `<OS_NAME>` must be one of the following values:
`ubuntu18.04`, `centos7.5` and `centos8.1`.

The resulting Docker image will have `occlum/occlum:<OCCLUM_LABEL>-<OS_NAME>` as its label.
