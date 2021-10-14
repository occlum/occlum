![Occlum logo](docs/images/logo.png)
## <!-- render a nicely looking grey line to separate the logo from the content  -->
[![All Contributors](https://img.shields.io/badge/all_contributors-7-orange.svg?style=flat-square)](CONTRIBUTORS.md)
[![Essential Test](https://github.com/occlum/occlum/workflows/Essential%20Test/badge.svg?branch=master)](https://github.com/occlum/occlum/actions?query=workflow%3A%22Essential+Test%22)
[![Demo Test](https://github.com/occlum/occlum/workflows/Demo%20Test/badge.svg?branch=master)](https://github.com/occlum/occlum/actions?query=workflow%3A%22Demo+Test%22)
<!--[![SGX Hardware Mode Test](https://github.com/occlum/occlum/workflows/SGX%20Hardware%20Mode%20Test/badge.svg?branch=master)](https://github.com/occlum/occlum/actions?query=workflow%3A%22SGX+Hardware+Mode+Test%22)-->

**NEWS:** Our paper _Occlum: Secure and Efficient Multitasking Inside a Single Enclave of Intel SGX_ has been accepted by [ASPLOS'20](https://asplos-conference.org/programs/). This research paper highlights the advantages of the single-address-space architecture adopted by Occlum and describes a novel in-enclave isolation mechanism that complements this approach. The paper can be found on [ACM Digital Library](https://dl.acm.org/doi/abs/10.1145/3373376.3378469) and [Arxiv](https://arxiv.org/abs/2001.07450).

Occlum is a *memory-safe*, *multi-process* library OS (LibOS) for [Intel SGX](https://software.intel.com/en-us/sgx). As a LibOS, it enables *legacy* applications to run on SGX with *little or even no modifications* of source code, thus protecting the confidentiality and integrity of user workloads transparently.

Occlum has the following salient features:

  * **Efficient multitasking.** Occlum offers _light-weight_ LibOS processes: they are light-weight in the sense that all LibOS processes share the same SGX enclave. Compared to the heavy-weight, per-enclave LibOS processes, Occlum's light-weight LibOS processes is up to _1,000X faster_ on startup and _3X faster_ on IPC. In addition, Occlum offers an optional _multi-domain [Software Fault Isolation](http://www.cse.psu.edu/~gxt29/papers/sfi-final.pdf) scheme_ to isolate the Occlum LibOS processes if needed.
  * **Multiple file system support.** Occlum supports various types of file systems, e.g., _read-only hashed FS_ (for integrity protection), _writable encrypted FS_ (for confidentiality protection), _untrusted host FS_ (for convenient data exchange between the LibOS and the host OS).
  * **Memory safety.** Occlum is the _first_ SGX LibOS written in a memory-safe programming language ([Rust](https://www.rust-lang.org/)). Thus, Occlum is much less likely to contain low-level, memory-safety bugs and is more trustworthy to host security-critical applications.
  * **Ease-of-use.** Occlum provides user-friendly build and command-line tools. Running applications on Occlum inside SGX enclaves can be as simple as only typing several shell commands (see the next section).

## Introduction

### Hello Occlum

If you were to write an SGX Hello World project using some SGX SDK, the project would consist of hundreds of lines of code. And to do that, you have to spend a great deal of time to learn the APIs, the programming model, and the build system of the SGX SDK.

Thanks to Occlum, you can be freed from writing any extra SGX-aware code and only need to type some simple commands to protect your application with SGX transparently---in four easy steps.

**Step 1. Compile the user program with the Occlum toolchain (e.g., `occlum-gcc`)**
```
$ occlum-gcc -o hello_world hello_world.c
$ ./hello_world
Hello World
```
Note that the Occlum toolchain is not cross-compiling in the traditional sense: the binaries built by the Occlum toolchain is also runnable on Linux. This property makes it convenient to compile, debug, and test user programs intended for Occlum.

**Step 2. Initialize a directory as the Occlum instance via `occlum init` or `occlum new`**
```
$ mkdir occlum_instance && cd occlum_instance
$ occlum init
```
or
```
$ occlum new occlum_instance
```
The `occlum init` command creates the compile-time and run-time state of Occlum in the current working directory. The `occlum new` command does basically the same thing but in a new instance diretory. Each Occlum instance directory should be used for a single instance of an application; multiple applications or different instances of a single application should use different Occlum instances.

**Step 3. Generate a secure Occlum FS image and Occlum SGX enclave via `occlum build`**
```
$ cp ../hello_world image/bin/
$ occlum build
```
The content of the `image` directory is initialized by the `occlum init` command. The structure of the `image` directory mimics that of an ordinary UNIX FS, containing directories like `/bin`, `/lib`, `/root`, `/tmp`, etc. After copying the user program `hello_world` into `image/bin/`, the `image` directory is packaged by the `occlum build` command to generate a secure Occlum FS image as well as the Occlum SGX enclave. The FS image is integrity protected by default, if you want to protect the confidentiality and integrity with your own key, please check out [here](docs/encrypted_image.md).

For platforms that don't support SGX, it is also possible to run Occlum in SGX simulation mode. To switch to the simulation mode, `occlum build` command must be given an extra argument or an environment variable as shown below:
```
$ occlum build --sgx-mode SIM
```
or
```
$ SGX_MODE=SIM occlum build
```

**Step 4. Run the user program inside an SGX enclave via `occlum run`**
```
$ occlum run /bin/hello_world
Hello World!
```
The `occlum run` command starts up an Occlum SGX enclave, which, behind the scene, verifies and loads the associated Occlum FS image, spawns a new LibOS process to execute `/bin/hello_world`, and eventually prints the message.

### Configure Occlum

Occlum can be configured easily via a configuration file named `Occlum.json`, which is generated by the `occlum init` command in the Occlum instance directory. The user can modify `Occlum.json` to configure Occlum. A sample of `Occlum.json` is shown below. Some comments are added to provide a brief explanation. If you are not sure how to set the `resource_limits` or `process` for your application, please check out [Resource Configuration Guide](docs/resource_config_guide.md).
```js
{
    // Resource limits
    "resource_limits": {
        // The total size of enclave memory available to LibOS processes
        "user_space_size": "256MB",
        // The heap size of LibOS kernel
        "kernel_space_heap_size": "32MB",
        // The stack size of LibOS kernel
        "kernel_space_stack_size": "1MB",
        // The max number of LibOS threads/processes
        "max_num_of_threads": 32
    },
    // Process
    "process": {
        // The stack size of the "main" thread
        "default_stack_size": "4MB",
        // The max size of memory allocated by brk syscall
        "default_heap_size": "16MB",
        // The max size of memory by mmap syscall
        "default_mmap_size": "32MB"
    },
    // Entry points
    //
    // Entry points specify all valid path prefixes for <path> in `occlum run
    // <path> <args>`. This prevents outside attackers from executing arbitrary
    // commands inside an Occlum-powered enclave.
    "entry_points": [
        "/bin"
    ],
    // Environment variables
    //
    // This gives a list of environment variables for the "root"
    // process started by `occlum exec` command.
    "env": {
        // The default env vars given to each "root" LibOS process. As these env vars
        // are specified in this config file, they are considered trusted.
        "default": [
            "OCCLUM=yes"
        ],
        // The untrusted env vars that are captured by Occlum from the host environment
        // and passed to the "root" LibOS processes. These untrusted env vars can
        // override the trusted, default envs specified above.
        "untrusted": [
            "EXAMPLE"
        ]
    },
    // Enclave metadata
    "metadata": {
        // Enclave signature structure's ISVPRODID field
        "product_id": 0,
        // Enclave signature structure's ISVSVN field
        "version_number": 0,
        // Whether the enclave is debuggable through special SGX instructions.
        // For production enclave, it is IMPORTANT to set this value to false.
        "debuggable": true
    },
    // Mount points and their file systems
    //
    // The default configuration is shown below.
    "mount": [
        {
            "target": "/",
            "type": "unionfs",
            "options": {
                "layers": [
                    {
                        "target": "/",
                        "type": "sefs",
                        "source": "./build/mount/__ROOT",
                        "options": {
                            "MAC": ""
                        }
                    },
                    {
                        "target": "/",
                        "type": "sefs",
                        "source": "./run/mount/__ROOT"
                    }
                ]
            }
        },
        {
            "target": "/host",
            "type": "hostfs",
            "source": "."
        },
        {
            "target": "/proc",
            "type": "procfs"
        },
        {
            "target": "/dev",
            "type": "devfs"
        }
    ]
}
```

### Try Experimental Features

1. Occlum has added several new experimental commands, which provide a more container-like experience to users, as shown below:
```
occlum init
occlum build
occlum start
occlum exec <cmd1> <args1>
occlum exec <cmd2> <args2>
occlum exec <cmd3> <args3>
occlum stop
```

2. Occlum has enabled per process resource configuration via `prlimit` syscall (https://man7.org/linux/man-pages//man2/prlimit.2.html) and shell built-in command `ulimit` (https://fishshell.com/docs/current/cmds/ulimit.html). For more info, please read [README.md](demos/fish/README.md) of `demos/fish`.

## How to Use?

We have built and tested Occlum on Ubuntu 18.04 with or without hardware SGX support (if the CPU does not support SGX, Occlum can be run in the SGX simulation mode). To give Occlum a quick try, one can use the Occlum Docker image by following the steps below:

Step 1-3 are to be done on the host OS (Linux):

1. Install [Intel SGX driver for Linux](https://github.com/intel/linux-sgx-driver), which is required by Intel SGX SDK.

2. Install [enable_rdfsbase kernel module](https://github.com/occlum/enable_rdfsbase), which enables Occlum to use `rdfsbase`-family instructions in enclaves.

3. Run the Occlum Docker container, which has Occlum and its demos preinstalled:

    For old IAS driver (not DCAP aware):
    ```bash
    docker run -it --device /dev/isgx occlum/occlum:[version]-ubuntu18.04
    ```

    For DCAP driver before v1.41:
    ```bash
    docker run -it --device /dev/sgx/enclave --device /dev/sgx/provision occlum/occlum:[version]-ubuntu18.04
    ```

    For DCAP driver since v1.41 or in-tree kernel driver:
    ```bash
    # Two methods:
    # (1) Create softlinks on host
    mkdir -p /dev/sgx
    cd /dev/sgx && ln -sf ../sgx_enclave enclave && ln -sf ../sgx_provision provision
    docker run -it --device /dev/sgx/enclave --device /dev/sgx/provision occlum/occlum:[version]-ubuntu18.04

    # (2) Create the docker with privileged mode
    docker run -it --privileged -v /dev/sgx_enclave:/dev/sgx/enclave -v /dev/sgx_provision:/dev/sgx/provision occlum/occlum:[version]-ubuntu18.04
    ```

Step 4-5 are to be done on the guest OS running inside the Docker container:

4. (Optional) Try the sample code of Intel SGX SDK to make sure that SGX is working
    ```
    cd /opt/intel/sgxsdk/SampleCode/SampleEnclave && make && ./app
    ```
5. Check out Occlum's demos preinstalled at `/root/demos`, whose README can be found [here](demos/README.md). Or you can try to build and run your own SGX-protected applications using Occlum as shown in the demos.

Alternatively, to use Occlum without Docker, one can install Occlum on popular Linux distributions like Ubuntu and CentOS with the Occlum DEB and RPM packages, respectively. These packages are provided for every release of Occlum since `0.16.0`. For more info about the packages, see [here](docs/install_occlum_packages.md).

## How to Build?

To build Occlum from the latest source code, do the following steps in an Occlum Docker container (which can be prepared as shown in the last section):

1. Download the latest source code of Occlum
    ```
    mkdir occlum && cd occlum
    git clone https://github.com/occlum/occlum .
    ```
2. Prepare the submodules and tools required by Occlum.
    ```
    make submodule
    ```
3. Compile and test Occlum
    ```
    make

    # test musl based binary
    make test

    # test glibc based binary
    make test-glibc

    # stress test
    make test times=100
    ```

    For platforms that don't support SGX
    ```
    SGX_MODE=SIM make
    SGX_MODE=SIM make test
    ```
4. Install Occlum
    ```
    make install
    ```
   which will install the `occlum` command-line tool and other files at `/opt/occlum`.

The Occlum Dockerfile can be found at [here](tools/docker/). Use it to build the container directly or read it to see the dependencies of Occlum.

## How to Build Occlum-Compatible Executable Binaries?

Occlum supports running any executable binaries that are 1) based on [musl libc](https://www.musl-libc.org/) and 2) position independent. We chose musl libc instead of Glibc since the codebase of musl libc is 10X smaller than Glibc, which means a much smaller Trusted Computing Base (TCB) and attack surface. We argue this is an important consideration for Occlum, which targets security-critical apps running inside SGX enclaves.

The two aforementioned requirements are not only satisfied by the Occlum toolchain, but also the native toolchains from some Linux distributions, e.g., [Alpine Linux](https://www.alpinelinux.org/). We think Alpine Linux, a popular Linux distribution that emphasizes simplicity and security, is a natural fit for Occlum. We have provided demos (see [Python](demos/python/)) to run unmodified apps from [Alpine Linux packages](https://pkgs.alpinelinux.org/packages).

## How to Debug?

To debug an app running upon Occlum, one can harness Occlum's builtin support for GDB via `occlum gdb` command. More info can be found [here](demos/gdb_support/).

Meanwhile, one can use `occlum mount` command to access and manipulate the secure filesystem for debug purpose. More info can be found [here](docs/occlum_mount.md).

If the cause of a problem does not seem to be the app but Occlum itself, then one can take a glimpse into the inner workings of Occlum by checking out its log. Occlum's log level can be adjusted through `OCCLUM_LOG_LEVEL` environment variable. It has six levels: `off`, `error`, `warn`, `debug`, `info`, and `trace`. The default value is `off`, i.e., showing no log messages at all. The most verbose level is `trace`.

## How to Build and Run Release-Mode Enclaves?

By default, the `occlum build` command builds and signs enclaves in debug mode. These SGX debug-mode enclaves are intended for development and testing purposes only. For production usage, the enclaves must be signed by a key acquired from Intel (a restriction that will be lifted in the future when Flexible Launch Control is ready) and run with SGX debug support disabled.

Occlum has built-in support for both building and running enclaves in release mode.
To do that, modify `Occlum.json` [metadata]-[debuggable] field to `false`. And then run the commands below:
```
$ occlum build --sign-key <path_to/your_key.pem>
$ occlum run <prog_path> <prog_args>
```

Ultimately, whether an enclave is running in the release mode should be checked and judged by a trusted client through remotely attesting the enclave. See the remote attestation demo [here](demos/remote_attestation).

## How to Run Occlum on Public Cloud?

To cut off the complexity of self-hosted infrastructure, one can deploy Occlum-powered SGX apps on public clouds with SGX support. For example, we have tested and successfully deployed Occlum Docker containers on [Azure Kubernetes Service (AKS)](https://azure.microsoft.com/en-us/services/kubernetes-service/#getting-started). Please check out [this doc](docs/azure_aks_deployment_guide.md) for more details.

## What is the Implementation Status?

Occlum is being actively developed. We now focus on implementing more system calls and additional features required in the production environment.

While this project is still not mature or stable (we are halfway through reaching version 1.0.0), we have used Occlum to port many real-world applications (like Tensorflow Lite, XGBoost, GCC, Lighttpd, etc.) to SGX with little or no source code modifications. We believe that the current implementation of Occlum is already useful to many users and ready to be deployed in some use cases.

## How about the Internal Working?

The high-level architecture of Occlum is summarized in the figure below:

![Arch Overview](docs/images/arch_overview.png)

## Why the Name?

The project name Occlum stems from the word *Occlumency* coined in Harry Potter series by J. K. Rowling. In *Harry Potter and the Order of Phoenix*, Occlumency is described as:

> The magical defence of the mind against external penetration. An obscure branch of magic, but a highly useful one... Used properly, the power of Occlumency will help shield you from access or influence.

The same thing can be said for Occlum, not for the mind, but for the program:

> The magical defence of the program against external penetration. An obscure branch of technology, but a highly useful one... Used properly, the power of Occlum will help shield your program from access or influence.

Of course, Occlum must be run on Intel x86 CPUs with SGX support to do its magic.

## Contributors

This project follows the [all-contributors](https://allcontributors.org) specification. Contributions of any kind are welcome! We will publish contributing guidelines and accept pull requests after the project gets more stable.

Thanks go to [all these wonderful contributors to this project](CONTRIBUTORS.md).

## License

Occlum is released by Ant Financial under BSD License. See the copyright information [here](LICENSE).
