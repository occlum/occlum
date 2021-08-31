# Mount and Unmount Filesystems at Runtime

## Background
Users can specify the mount configuration in the `Occlum.json` file, then the filesystems are mounted during the libOS startup phase. While this design provides a safe and simple way to access files, it is not as convenient as traditional Host OS. Apps are not allowd to mount and unmount filesystems at runtime.

## How to mount filesystems at runtime?
Apps running inside Occlum can mount some specific filesystems via the [mount()](https://man7.org/linux/man-pages/man2/mount.2.html) system call. This makes it flexible to mount and access files at runtime.

Currently, we only support to create a new mount with the trusted UnionFS consisting of SEFSs or the untrusted HostFS. The mountpoint is not allowd to be the root directory("/").

### 1. Mount trusted UnionFS consisting of SEFSs
Example code:

```
mount("unionfs", "<target_dir>", "unionfs", 0/* mountflags is ignored */,
      "lowerdir=<lower>,upperdir=<upper>,key=<128-bit-key>")
```

Mount options:

- The `lowerdir=<lower>` is a mandatory field, which describes the directory path of the RO SEFS on Host OS.
- The `upperdir=<upper>` is a mandatory field, which describes the directory path of the RW SEFS on Host OS.
- The `key=<128-bit-key>` is an optional field, which describes the 128bit key used to encrypt or decrypt the FS. Here is an example of the key: `key=c7-32-b3-ed-44-df-ec-7b-25-2d-9a-32-38-8d-58-61`. If this field is not provided, it will use the automatic key derived from the enclave sealing key.

### 2. Mount untrusted HostFS
Example code:

```
mount("hostfs", “<target_dir>”, "hostfs", 0/* mountflags is ignored */,
      "dir=<host_dir>")
```

Mount options:

- The `dir=<host_dir>` is a mandatory field, which describes the directory path on Host OS.

## How to unmount filesystems at runtime?

Apps running inside Occlum can unmount some specific filesystems via the [umount()/umount2()](https://man7.org/linux/man-pages/man2/umount.2.html) system calls. Note that root directory("/") is not allowd to unmount.

Example code:
```
umount("<target_dir>")
```