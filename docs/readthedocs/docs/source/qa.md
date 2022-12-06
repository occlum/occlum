# Q&A

### Does an Occlum instance directory correspond to only one binary, or does an Occlum instance directory contain multiple binaries? If yes, do they all belong to the security zone?

Occlum is a multiprocess LibOS, which means that a user could add multiple executables into one Occlum instance and all those applications are able to work together. Technically, all those applications are running in the same Enclave, so there are in the same security zone.

### If there are two executables in an Occlum instance, is it possible to execute both of them by using occlum_pal_exec()?

The short answer is yes. But the user could only run one of them by occlum_pal_exec() at a time. If you want to run both of them at the same time, one of the application could spawn the other, or you could prepare a script to launch them in the script one by one.

### Is there a way to share memory between an Enclave with Occlum instance?

No, Occlum does not support shared memory with other Occlum instances or enclaves.

### How many CPU cores can be used inside Occlum/Enclave?

The host OS manages the CPU resource, and doing the scheduling. So it is totally controlled by host OS that how many cpus are running the applications inside Occlum.

### Can Occlum support running network related applications?

Yes. Generally, the network related applications can run successfully in Occlum without modification. Just one note, besides the application itself, multiple files/directories may be required to be existed in Occlum image as well. For example,

* **`hostname`, `hosts` and `resolv.conf` files**

Generally, these files (in the host environment) are automatically parsed and transferred to Occlum LibOS for each `Occlum run` operation.

* **DNS related files**

Add below part in the bom file if required.
```
  - target: /opt/occlum/glibc/lib
    copy:
      - files:
          - /opt/occlum/glibc/lib/libnss_files.so.2
          - /opt/occlum/glibc/lib/libnss_dns.so.2
          - /opt/occlum/glibc/lib/libresolv.so.2
```

* **CA related files**

Add below part in the bom file if required.
```bom.yaml
  - target: /etc
    copy:
      - dirs:
        - /etc/ssl
```
