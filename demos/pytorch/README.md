# Use Pytorch with Occlum

This project demonstrates how Occlum enables [Pytorch](https://pytorch.org/) programs running in SGX enclaves.

## Acknowledgement: 

The sample training script and Alpine-Pytorch Dockerfile used here are referenced form [SGX-LKL](https://github.com/lsds/sgx-lkl).

## How to Run

This tutorial is written under the assumption that you have Docker installed and use Occlum in a Docker container.

Occlum is compatible with native binaries from Alpine Linux, so we can prepare an Alpine Pytorch Docker image and copy the rootfs of it, then run training script directly on Occlum.

Step 1 (on the host): Start an Alpine Linux container
```
docker build -t alpine-pytorch .
docker run --name "<alpine_container_name>" alpine-pytorch
```

Step 2 (on the host): Copy the `import_alpine_pytorch.sh` script from Occlum container to host,
```
docker cp "<occlum_container_name>":/root/demos/pytorch/import_alpine_pytorch.sh <host_dir>
```
and import the rootfs of Alpine Linux Docker image to the Occlum container (`/root/alpine_python`)
```
./<host_dir>/import_alpine_pytorch.sh "<alpine_container_name>" "<occlum_container_name>"
```

Step 3 (in the Occlum container): Run the sample code on Occlum via
```
./run_pytorch_on_occlum.sh
```
I only test this in SIM mode, since my device doesn't support HW mode.
