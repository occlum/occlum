# Build and Install

Generally, users don't need build the Occlum from source. They can directly use Occlum official docker image in Docker hub.
```
docker pull occlum/occlum:[version]-ubuntu20.04
```

## Build from Source

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

If `release` build and install is required, just add **OCCLUM_RELEASE_BUILD=1** in front of every `make` command.   
The Occlum Dockerfile can be found at [`here`](https://github.com/occlum/occlum/tree/master/tools/docker). Use it to build the container directly or read it to see the dependencies of Occlum.

## How to Debug?

To debug an app running upon Occlum, one can harness Occlum's builtin support for GDB via `occlum gdb` command. More info can be found [`here`](https://github.com/occlum/occlum/tree/master/demos/gdb_support).

Meanwhile, one can use `occlum mount` command to access and manipulate the secure filesystem for debug purpose.

If the cause of a problem does not seem to be the app but Occlum itself, then one can take a glimpse into the inner workings of Occlum by checking out its log. Occlum's log level can be adjusted through `OCCLUM_LOG_LEVEL` environment variable. It has six levels: `off`, `error`, `warn`, `debug`, `info`, and `trace`. The default value is `off`, i.e., showing no log messages at all. The most verbose level is `trace`.

## How to Build and Run Release-Mode Enclaves?

By default, the `occlum build` command builds and signs enclaves in debug mode. These SGX debug-mode enclaves are intended for development and testing purposes only. For production usage, the enclaves must be signed by a key acquired from Intel (a restriction that will be lifted in the future when Flexible Launch Control is ready) and run with SGX debug support disabled.

Occlum has built-in support for both building and running enclaves in release mode.
To do that, modify `Occlum.yaml` [metadata]-[debuggable] field to `false`. And then run the commands below:
```
$ occlum build --sign-key <path_to/your_key.pem>
$ occlum run <prog_path> <prog_args>
```

Ultimately, whether an enclave is running in the release mode should be checked and judged by a trusted client through remotely attesting the enclave. See the remote attestation demo [here](demos/remote_attestation).

## The Encrypted FS Image

Occlum has supported using an encrypted FS image, which is encrypted by a user-provided key, to run apps inside the enclave. The confidentiality and integrity of user's files and libraries are both protected with it.

To generate the encrypted FS image, user must give the `--image-key <key_path>` flag in the `occlum build` command (If the flag is not given, the secure FS image will be integrity protected only). The `<key_path>` refers to a file consisting of a 128-bit key and the user can generate it via the `occlum gen-image-key <key_path>` command.
