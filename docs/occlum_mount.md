# The Mount Command

The `occlum mount` command is designed to mount the secure FS image of the Occlum instance as a Linux FS at a specified path on Linux. This makes it easy to access and manipulate Occlum's secure FS for debug purpose.

## Prerequisites

The command depends on Linux's [Filesystem in Userspace (FUSE)](https://en.wikipedia.org/wiki/Filesystem_in_Userspace), which consists of two components: a kernel module and a userspace library. In most of Linux distributions, the FUSE kernel module is part of the Linux kernel by default, so it should not be a problem. The FUSE library can be installed via a package manager (e.g., `sudo apt-get install libfuse-dev` on Ubuntu). You do not need to install the package manually when using the Occlum Docker image as it has preinstalled the package.

One common pitfall when using FUSE with Docker is about privilege. A Docker container, by default, does not have the privilege to use FUSE. To get the privilege, the Docker container must be started with the following flags:
```
--cap-add SYS_ADMIN --device /dev/fuse
```
or
```
--privileged
```
For more info about enabling FUSE for Docker, see [here](https://github.com/docker/for-linux/issues/321).

## How to use

To mount an Occlum's secure FS image successfully, three conditions must be satisfied:

1. The secure FS image exists and is not being used (e.g., via the `occlum run` or `occlum mount` command). This condition ensures that the secure FS will not be broken due to the concurrent access of different Occlum commands.

2. The three (optional) sign key, sign tool and image key arguments that are given to the `occlum mount` command must have the same values as those given to the `occlum build` command, which is how the image is created in the first place. This ensures that the secure FS can only be accessed by the owner of the enclave.

3. If the image key is not given to the `occlum build` command, the `occlum mount` command must be run on the same machine as the `occlum run` command that runs the current Occlum instance and writes to the image. This condition is due to the fact that the encryption key of the secure FS is bound to the machine.

With the three conditions satisfied, the mount command is able to start a Linux FUSE FS server. Any I/O operations on the FUSE FS mounted at the specified path will be redirected by Linux kernel as I/O requests to the FUSE server. The FUSE server is backed by a special enclave, which can encrypt or decrypt the content of the secure FS image on demand.

The mount command **will not return** until the FUSE server is terminated in one of the two ways. The first one is to press ctrl+C. The second one is to use `umount` command. Both ways can terminate the server gracefully.

Step 1: Create an empty directory to serve as the mount point
```
mkdir <path>
```

Step 2: Mount the secure FS at the newly created mount point
```
occlum mount [--sign-key <key_path>] [--sign-tool <tool_path>] [--image-key <key_path>] <path>
```
After mounting the secure FS successfully, you can access and manipulate the FS from the mount point as easy as regular Linux FS.
