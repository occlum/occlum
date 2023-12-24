# PKU Feature in Occlum

Occlum now can use PKU, a hardware feature, to enforce isolation between LibOS and (LibOS's)userspace applications. Here is its manual.

## Background and Motivation

**Note**: Userspace applications in this manual always refers to the LibOS's userpsace applications.

**PKU** (Protection Keys for Userspace) (aka. **MPK**: Memory Protection Keys) has been introduced into Linux since 2015 (details can be found in the [lwn page](https://lwn.net/Articles/643797/)). It is a lightweight intra-process isolation mechanism for userspace (Ring 3) software. Since its memory access policy is restricted by MMU (Memory Management Unit), it incurs almost non-zero overhead at runtime compared to Software Fault Isolation (SFI), and the memory access permission switch overhead is low. More details can be found in the [manual page](https://man7.org/linux/man-pages/man7/pkeys.7.html).

Currently, Occlum lacks the ability to isolate LibOS from userspace applications. Though userspace applications are **considered benign** in Occlum, but it is bug-prone inevitably. Potential illegal memory accesses may affect the correctness of computation silently, even lead to the crash of the whole enclave. Intra-enclave isolation is helpful for developers to uncover bugs beforehand.

It necessary to enforce the isolation in Occlum, and leveraging PKU is a good choice.

### Security Analysis
PKU is an option for users and developers to enhance Occlum's fault isolation between LibOS and userspace applications.
It is not a complete protection from malicious attacks which come from an enclave itself.
**Userspace applications are still in TCB (Trusted Computing Base) in Occlum's threat model.**
They are considered to be benign but inevitably bug-prone.

It is should be emphasized that OS has the full control of enclave's page table and control registers (e.g. CR4), and OS has mainly two ways to enforce data access policy related to PKU:

1. OS is able to set CR4.PKE to 0, rendering intra-enclave isolation useless;
2. OS is able to misconfigure pkeys in PTE.

So OS is able to perform attacks easily by misconfiguring PTEs or CR4. However, such configurations can only lead to DoS (Denial-of-Service) attcks, and DoS is not considered in SGX's threat model. If users worry that PKU feature in Occlum opens a new attack vector, they can turn off PKU feature in production environment. Users can still turn on PKU feature in test environment before releasing their APPs.

## Requirement

1. The CPU supports PKU feature, which can checked by:

    ```bash
    cat /proc/cpuinfo | grep pku
    ```

2. OS has set CR4.PKE to 1, which can be checked by:

    ```bash
    cat /proc/cpuinfo | grep ospke
    ```

3. Occlum relies on syscalls to configure PKU:

    - `pkey_alloc()`
    - `pkey_mprotect()`
    - `pkey_free()`

    Occlum always runs in docker environment. Docker uses secure computing mode (`seccomp`) provided by Linux to specify the available within the container ([details](https://docs.docker.com/engine/security/seccomp/)). Currently, the default `seccomp` profile has not added PKU related syscall into its white list (we have reported such [issue](https://github.com/moby/moby/issues/43481)). To work around it, developers needs to add `--privileged` flag in `docker run` command, or use our customized [profile](https://github.com/Bonjourz/moby/blob/43481_support_pku/profiles/seccomp/default.json) by adding [`--security-opt`](https://docs.docker.com/engine/security/seccomp/#pass-a-profile-for-a-container) flag.

    To check whether PKU related syscalls are allowed in container environment, users can run sample code **in Occlum's container environment**:

    ```c
    #define _GNU_SOURCE
    #include <stdlib.h>
    #include <stdio.h>
    #include <sys/mman.h>

    #define BUF_SIZE    (0x1000)

    int main() {
        int ret = -1, pkey = -1;
        ret = pkey_alloc(0, 0);
        if (ret < 0) {
            perror("Cannot invoke pkey_alloc() successfully");
            exit(-1);
        } else {
            pkey = ret;
            char *buf = (char *)mmap(NULL, BUF_SIZE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
            if (buf == MAP_FAILED) {
                perror("Cannot invoke mmap() successfully");
                exit(-1);
            }
            ret = pkey_mprotect(buf, BUF_SIZE, PROT_READ | PROT_WRITE, pkey);
            if (ret < 0) {
                perror("Cannot invoke pkey_mprotect() successfully");
                exit(-1);
            }
            ret = munmap(buf, BUF_SIZE);
            if (ret < 0) {
                perror("Cannot invoke munmap() successfully");
                exit(-1);
            }
            ret = pkey_free(pkey);
            if (ret < 0) {
                perror("Cannot invoke pkey_free() successfully");
                exit(-1);
            }
        }
        printf("PKU related syscalls are allowed in container environment!\n");
        return 0;
    }
    ```

    The following output:

    ```text
    PKU related syscalls are allowed in container environment!
    ```

    indicates PKU related syscalls are allowed in container environment.

## How to Configure it

PKU feature can only be enabled in `HW` mode. Whether to turn on PKU feature is controlled by `Occlum.json`.

```js
{
    // ...
    "feature": {
        // "pkru" = 0: PKU feature must be disabled
        // "pkru" = 1: PKU feature must be enabled
        // "pkru" = 2: PKU feature is enabled if the platform supports it
        "pkru": 0
    }
    // ...
}
```

Users have three options for PKU feature:

1. If `feature.pkru` == 0: The PKU feature must disabled in Occlum. It is the default value. Since docker has not supported PKU related syscalls in container environment by default, Occlum turns off PKU feature in default configuration.

2. If `feature.pkru` == 1: The PKU feature must enabled in Occlum. If CPU on the platform does not support PKU, then enclave cannot be successfully initialized.

3. If `feature.pkru` == 2: If CPU supports PKU, also OS enables PKU feature, PKU feature is turned on in Occlum.
