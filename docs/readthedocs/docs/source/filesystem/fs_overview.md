# Occlum File System Overview

Occlum supports various file systems: e.g., read-only integrity-protected SEFS, writable encrypted SEFS, UnionFS, Async-SFS, untrusted HostFS, RamFS, and other pseudo filesystems.

Here is the default FS layout:

```
                  │"/"
           ┌──────┴──────┐
           │   UnionFS   │
           │ ┌─────────┐ │
           │ │ SEFS(RW)│ │
           │ ├─────────┤ │
           │ │ SEFS(RO)│ │
           │ └─────────┘ │
           │             │
           └──────┬──────┘
                  │
    ┌────────┬────┴─────┬────────┐
    │        │          │        │
    │"/sfs"  │"/dev/shm"│"/proc" │"/dev"
┌───┴─┐   ┌──┴──┐   ┌───┴──┐  ┌──┴──┐
│A-SFS│   │RamFS│   │ProcFS│  │DevFS│
└─────┘   └─────┘   └──────┘  └─────┘
```

## SEFS
The SEFS is a filesystem based on the [Intel SGX Protected File (PFS)](https://www.intel.com/content/www/us/en/developer/articles/technical/overview-of-intel-protected-file-system-library-using-software-guard-extensions.html), it protects the integrity and confidentiality of disk I/O data from the host.

Here is the hierarchy of SEFS:
```
  ┌──────────┐
  │   SEFS   │
  │   Inode  │
  └────┬─────┘
       │
 ┌─────┴──────┐
 │Rust SGX SDK│
 │   SGXFile  │
 └─────┬──────┘
       │
┌──────┴────────┐
│ Intel SGX SDK │
│ Protected File│
└───────────────┘
```

There are two modes for SEFS:
1. Integrity-only mode
2. Encryption mode

### Integrity-only mode
We modified the Intel SGX PFS to add this mode. It only protects the integrity of FS, which will generate the deterministic hash for the same FS image. So it is convenient to implement the remote attestation for the enclave with the same FS image.

The FS image (the `image` dir of occlum instance ) provided by the user via the `copy_bom` tool will be transformed into a SEFS image in this mode by default. For the use of `copy_bom`, please refer to [this](https://occlum.readthedocs.io/en/latest/tools/copy_bom.html).

### Encryption mode
The integrity and confidentiality of the FS are both protected in this mode. There are two key-generation policies for this mode: the autokey generation policy and the user-provided key policy.

* The autokey generation policy

In this policy mode, the user is not required to provide the key. The key is automatically derived from the MRSIGNER of the enclave, the ProdID of the enclave, and the hardware info. So the same owner of two enclaves can share the FS data on the same machine.
This policy is the default one for the encryption mode of SEFS.

* The user-provided policy

In this policy mode, the key should be provided by the user, which means the enclave owner should manage the key. This policy is more flexible for the user to control the data for sharing or isolation. The [doc](https://occlum.readthedocs.io/en/latest/filesystem/encrypted_image.html) shows you how to use this policy mode.

## UnionFS
As you can tell, we use the UnionFS consisting of SEFS as the rootfs of LibOS. To attest to the integrity of the user-provided FS image while having the ability to write data when running apps, we introduce a filesystem called UnionFS to satisfy this requirement.

UnionFS allows files and directories of separate file systems, known as branches,  to be transparently overlaid, forming a single coherent file system.

We use two SEFSs to form the UnionFS. The lower layer is the read-only(RO) SEFS in integrity-only mode, and the upper is the writable(RW) SEFS in encryption mode. Generally speaking, the RO-SEFS is transformed by the `image` dir provided by the user while building the enclave, and the RW-SEFS is generated while the enclave is running.

```
┌─────────────┐
│   UnionFS   │
│ ┌─────────┐ │
│ │ SEFS(RW)│ │
│ ├─────────┤ │
│ │ SEFS(RO)│ │
│ └─────────┘ │
│             │
└─────────────┘
```

Here is the configuration of rootfs, the first item is the lower layer RO-SEFS and the second item is the upper layer RW-SEFS. As you can tell, the RO-SEFS is at `./build/mount/__ROOT` and the RW-SEFS is at `./run/mount/__ROOT`.
```
  - target: /
    type: unionfs
    options:
      layers:
        # The read-only layer which is generated in `occlum build`
        - target: /
          type: sefs
          source: ./build/mount/__ROOT
          options:
            MAC: ''
        # The read-write layer whose content is produced when running the LibOS
        - target: /
          type: sefs
          source: ./run/mount/__ROOT
```

## Async-SFS
The Async-SFS is an asynchronous filesystem, which uses Rust asynchronous programming skills, making it fast and concurrent. It is mounted at `/sfs` by default. To achieve the high-performanced security, it uses the JinDisk as the underlying data storage and sends async I/O requests to it.

To accelerate block I/O, the page cache is introduced. It caches all the block I/O in the middle of Async-SFS and JinDisk. Thanks to the page cache and JinDisk, the result of the benchmark (e.g., FIO and Filebench) is significantly better than SEFS. If your App's performance is highly dependent on disk I/O, it is recommended to use Async-SFS.
```
┌───────────┐
│           │
│ Async-SFS │
│           │
└─────┬─────┘
      │
┌─────┴─────┐
│           │
│ Page Cache│
│           │
└─────┬─────┘
      │
┌─────┴─────┐
│           │
│  JinDisk  │
│           │
└───────────┘
```

Currently, there are some limitations of Async-SFS:
1. The maximum size of the file is 4GB.
2. The maximum size of FS is 16TB.

## HostFS
The HostFS is used for convenient data exchange between the LibOS and the host OS. It simply wraps the untrusted host OS file to implement the functionalities of FS. So the data is straightforwardly transferred between LibOS and host OS without any protection or validation.

## RamFS and other pseudo filesystems
The RamFS and other pseudo filesystems like ProcFS use the memory as the storage. So the data may lose if one terminates the enclave.

Please remember to enlarge the `kernel_space_heap_size` of Occlum.yaml if your app depends on RamFS.

## Q & A

### How to decrypt and view the rootfs?
One can use the `occlum mount` command to implements. Please refer to this [doc](https://occlum.readthedocs.io/en/latest/filesystem/mount.html#mount-command) for more information.

### How to mount FS at runtime?
Please refer to this [doc](https://occlum.readthedocs.io/en/latest/filesystem/mount.html#mount-and-unmount-filesystems-at-runtime).
