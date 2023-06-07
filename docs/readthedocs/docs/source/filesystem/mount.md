# Mount Support

## Mount command

The `occlum mount` command is designed to mount the secure FS image of the Occlum instance as a Linux FS at a specified path on Linux. This makes it easy to access and manipulate Occlum's secure FS for debug purpose.

### Prerequisites

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

### How to use

To mount an Occlum's secure FS image successfully, three conditions must be satisfied:

1. The secure FS image exists and is not being used (e.g., via the `occlum run` or `occlum mount` command). This condition ensures that the secure FS will not be broken due to the concurrent access of different Occlum commands.

2. The three (optional) sign key, sign tool and image key arguments that are given to the `occlum mount` command must have the same values as those given to the `occlum build` command, which is how the image is created in the first place. This ensures that the secure FS can only be accessed by the owner of the enclave.

3. If the image key is not given to the `occlum build` command, the `occlum mount` command must be run on the same machine as the `occlum run` command that runs the current Occlum instance and writes to the image. This condition is due to the fact that the automatically drived encryption key of the secure FS is bound to the machine, i.e., the MRSIGNER key policy.

With the three conditions satisfied, the mount command is able to start a Linux FUSE FS server. Any I/O operations on the FUSE FS mounted at the specified path will be redirected by Linux kernel as I/O requests to the FUSE server. The FUSE server is backed by a special enclave, which can encrypt or decrypt the content of the secure FS image on demand.

Please note that if the `autokey_policy` field of the configurations of FS is set in Occlum.yaml, the mount command will not work. This is because the MRENCLAVE is used as an input to generate the encryption key, and the mount tool cannot mimic it.

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


## Mount and Unmount Filesystems at Runtime

### Background
Users can specify the mount configuration in the `Occlum.yaml` file, then the file systems are mounted during the libOS startup phase. While this design provides a safe and simple way to access files, it is not as convenient as traditional Host OS. Apps are not allowed to mount and unmount file systems at runtime.

### How to mount filesystems at runtime?
Apps running inside Occlum can mount some specific file systems via the [mount()](https://man7.org/linux/man-pages/man2/mount.2.html) system call. This makes it flexible to mount and access files at runtime.

Currently, we only support to create a new mount with the trusted UnionFS consisting of SEFSs or the untrusted HostFS. The mount point is not allowed to be the root directory("/").

#### 1. Mount trusted UnionFS consisting of FSs
Example code:

```
mount("unionfs", "<target_dir>", "unionfs", 0/* mountflags is ignored */,
      "lowerdir=<lower>,lowerfs=<fs_type>,upperdir=<upper>,upperfs=<fs_type>,key=<128-bit-key>,sfssize=<size>,cachesize=<size>")
```

Mount options:

- The `lowerdir=<lower>` is a mandatory field, which describes the directory path of the RO FS on Host OS.
- The `lowerfs=<fs_type>` is a mandatory field, which describes the type of the RO FS. (Support SEFS/AsyncSFS by now)
- The `upperdir=<upper>` is a mandatory field, which describes the directory path of the RW FS on Host OS.
- The `upperfs=<fs_type>` is a mandatory field, which describes the type of the RW FS. (Support SEFS/AsyncSFS by now)
- The `key=<128-bit-key>` is an optional field, which describes the 128bit key used to encrypt or decrypt the FS. Here is an example of the key: `key=c7-32-b3-ed-44-df-ec-7b-25-2d-9a-32-38-8d-58-61`. If this field is not provided, it will use the automatic key derived from the enclave sealing key.
- The `sfssize=<size>` is an optional field, which describes the total size of AsyncSFS.
- The `cachesize=<size>` is an optional field, which describes the size of page cache used by AsyncSFS.

#### 2. Mount untrusted HostFS
Example code:

```
mount("hostfs", “<target_dir>”, "hostfs", 0/* mountflags is ignored */,
      "dir=<host_dir>")
```

Mount options:

- The `dir=<host_dir>` is a mandatory field, which describes the directory path on Host OS.

### How to unmount filesystems at runtime?

Apps running inside Occlum can unmount some specific file systems via the [umount()/umount2()](https://man7.org/linux/man-pages/man2/umount.2.html) system calls. Note that root directory("/") is not allowed to unmount.

Example code:
```
umount("<target_dir>")
```

