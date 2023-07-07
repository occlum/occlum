# Occlum-Compatible Executable Binaries

The `hello_world` demo is based on [musl libc](https://www.musl-libc.org/) with recompiling. But Occlum actually can support both `musl libc` and `glibc` based executable binaries without recompiling if they meet below three principles.

## No fork syscall

By design, Occlum doesn't support `fork` syscall. If there is `fork` syscall in the application, users have to assess if the `fork` could be replaced by `vfork + exec` or `posix spawn`. If yes, code modification and recompiling is inevitable.

## libc version compatibility

No recompiling doesn't mean the original `libc` libraries can be directly used in Occlum. To run in Occlum TEE environment, customized `libc` libraries are provided in the Occlum development docker image.

|    libc   |  Compatible Version in Occlum  | Path in Occlum Docker Image |
| --------- | ------------------------------ | --------------------------- |
| musl libc | <=1.1.24<br>(default version in Alpine:3.11) | /usr/local/occlum/x86_64-linux-musl/lib/ |
|   glibc   | <=2.31<br>(default version in Ubuntu:20.04)  |         /opt/occlum/glibc/lib/           |

Actually, the original `libc` libraries are to be replaced silently in `Occlum build` stage by `copy_bom` tool.

## Compiled with PIE (Position-Independent-Executable)

Current Ubuntu:20.04 and Alpine:3.11 enable `PIE` in default.
