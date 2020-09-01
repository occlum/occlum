# The Mount Command

The `occlum mount` command is designed to mount the secure FS image of the Occlum instance as a Linux FS at a specified path on Linux. This makes it easy to access and manipulate Occlum's secure FS for debug purpose.

To mount an Occlum's secure FS image successfully, three conditions must be satisfied:

1. The secure FS image exists and is not being used (e.g., via the `occlum run` or `occlum mount` command). This condition ensures that the secure FS will not be broken due to the concurrent access of different Occlum commands.

2. The two (optional) sign key and sign tool arguments that are given to the `occlum mount` command must have the same values as those given to the `occlum build` command, which is how the image is created in the first place. This ensures that the secure FS can only be accessed by the owner of the enclave.

3. The `occlum mount` command must be run on the same machine as the `occlum run` command that runs the current Occlum instance and writes to the image. This condition is due to the fact that the encryption key of the secure FS is bound to the machine.

With the three conditions satisfied, the mount command is able to start a Linux FUSE (Filesystems in Userspace) FS server. Any I/O operations on the FUSE FS mounted at the specified path will be redirected by Linux kernel as I/O requests to the FUSE server. The FUSE server is backed by a special enclave, which can encrypt or decrypt the content of the secure FS image on demand.

The mount command **will not return** until the FUSE server is terminated in one of the two ways. The first one is to press ctrl+C. The second one is to use `umount` command. Both ways can terminate the server gracefully.

## How to use

Step 1: Create an empty directory to serve as the mount point
```
mkdir <path>
```

Step 2: Mount the secure FS at the newly created mount point
```
occlum mount [--sign-key <key_path>] [--sign-tool <tool_path>] <path>
```
After mounting the secure FS successfully, you can access and manipulate the FS from the mount point as easy as regular Linux FS.
