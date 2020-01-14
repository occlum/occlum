# Use Python with Occlum

This project demonstrates how Occlum enables [Python](https://www.python.org) programs running in SGX enclaves.

Occlum is compatible with native binaries from Alpine Linux, so we can copy the Python from Alpine Linux and run it directly on Occlum.

Step 1: Copy the `import_alpine_python.sh` script from an Occlum container to host
```
docker cp <occlum_container>:/root/demos/python/import_alpine_python.sh <host_dir>
```
The script downloads a Docker image of Alpine Linux with Python preinstalled and imports the rootfs of the image into an Occlum container so that later we can copy the Alpine's Python libraries into an Occlum secure FS image.

Step 2: Import the rootfs of Alpine Linux's Python Docker image from host to the Occlum container (`/root/alpine_python`)
```
./<host_dir>/import_alpine_python.sh <occlum_container>
```

Step 3: You can attach to the Occlum container and run a `hello.py` sample on Occlum via
```
./run_python_on_occlum.sh
```
