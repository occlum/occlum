# Use Python with Occlum

This project demonstrates how Occlum enables _unmodified_ [Python](https://www.python.org) programs running in SGX enclaves.

## Sample Code: CSV Processing in Python

To make the sample code more realistic, we choose to write a Python program that processes CSV data files using [NumPy](https://numpy.org), [pandas](https://pandas.pydata.org), and [scikit-learn](https://scikit-learn.org). The sample code can be found [here](demo.py).

## How to Run

This tutorial is written under the assumption that you have Docker installed and use Occlum in a Docker container.

Occlum is compatible with native binaries from Alpine Linux, so we can prepare an Alpine Python Docker image and copy the rootfs of it, then run Python code directly on Occlum.

Step 1 (on the host): Start an Alpine Linux container with Python preinstalled
```
docker pull python:3.7-alpine3.10
docker run -it --entrypoint /bin/sh --name "<alpine_container_name>" python:3.7-alpine3.10
```

Step 2 (in the Alpine container): Install the required Python modules
```
apk add g++ lapack-dev gfortran
pip3 install numpy pandas scipy==1.3.1 Cython scikit-learn==0.21.1
```
Now that we have installed the required Python libraries in the Alpine Docker image, we can copy the content of the Alpine Docker image into the Occlum container so that we can build a trusted Occlum FS image with Alpine's Python installation inside.

Step 3 (on the host): Copy the `import_alpine_python.sh` script from Occlum container to host,
```
docker cp "<occlum_container_name>":/root/demos/python/import_alpine_python.sh <host_dir>
```
and import the rootfs of Alpine Linux Docker image to the Occlum container (`/root/alpine_python`)
```
./<host_dir>/import_alpine_python.sh "<alpine_container_name>" "<occlum_container_name>"
```

Step 4 (in the Occlum container): Run the sample code on Occlum via
```
./run_python_on_occlum.sh
```
It will process CSV data files and generate a file (`smvlight.dat`) in `./occlum_instance`.
