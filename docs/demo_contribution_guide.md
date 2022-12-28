# How To Contribute A Demo To Occlum

Occlum is used more and more widely by users with different needs from various domains. The team is planning on validating and supporting all mainstream applications including big data, AI, database, KMS, etc. However, we still could miss some application which is vital to some of the users. If a user would like to see an application to support very soon and get tested for every commit and release, contributions in all kinds are always welcome. This document can be found useful when a user wants to contribute a demo to Occlum.

Questions and issues are welcome if you meet any trouble during enabling the application.

## Step 1: Test on Local Machine

Please follow [Occlum README doc](https://github.com/occlum/occlum#how-to-use) to establish a valid Occlum environment. And make sure `hello_world` example can run. If there is no SGX hardware, you can also run with `SGX_MODE=SIM` to run in SGX simulation mode.

### 1.1 - Create Occlum instance

Occlum is a LibOS targeting security with best user experience. Occlum now supports both musl libc and Glibc. To use musl libc or Glibc, that is the question. We chose musl libc as our default libc due to its small trusted computing base (TCB), which is highly valued for security-critical applications that are intended to run inside enclaves. But since most programs are compiled with Glibc, this means the users have to recompile their programs with Occlum's musl-based C toolchain to run them on Occlum. Three methods are listed below for users to create an Occlum instance.

#### 1.1.1 - Rebuild with musl

For applications with strong security requirements, it is recommended to rebuild with musl. And then copy the binary and all the dependencies to Occlum instance. Most of the demos existed are running in this way. The [https server demo](https://github.com/occlum/occlum/tree/master/demos/https_server) can be used as a reference for this case.

#### 1.1.2 - Take from Alpine Linux

While recompiling does not appear to be a big deal at first, it could easily become a big headache for medium-to-large projects, which usually relies on a complex build system that is hard to understand or change. In this case, taking binaries and dependencies from Alpine Linux is another choice. [Alpine Linux](https://alpinelinux.org/) is a Linux distribution based on ` musl ` and BusyBox, designed for security, simplicity, and resource efficiency. Occlum has a natural born compatibility to binaries in Alpine Linux thanks to ` musl`. Thus, users could install applications with Alpine Linux package manager and copy the binaries and dependencies to Occlum instance. The [python demo](https://github.com/occlum/occlum/tree/master/demos/python) can be used as a reference.

#### 1.1.3 - Just use Glibc version

If the two methods above don’t work for you, Occlum also supports application compiled with Glibc for widest compatibility. Note that there are still two implicit requirements for binary compatibility with Occlum:
- [ ] The executables must be built as `position independent executables`. Fortunately, most distributions have already configured their GCC toolchains with `-PIE` by default for its security benefits.
- [ ] The executables must be dynamically linked with Glibc. Again, this is also the default behavior of GCC.

The [Flink demo](https://github.com/occlum/occlum/tree/master/demos/flink) can be used as a reference for this case.

### 1.2 Tune the configuration file

After the application binaries and the dependencies are copied to the Occlum instance, we can try to run the instance.

For the first time, it is likely to fail because the default configuration file doesn't provide enough resources for large applications. Users can refer to these two documents [[1]](https://github.com/occlum/occlum#configure-occlum) [[2]](https://github.com/occlum/occlum/blob/master/docs/resource_config_guide.md) to modify the configuration items accordingly.


## Step 2: Write Script for Easy Reproduction

To test and reproduce the new demo, it is better to "script-ize" the work. For example, a complete demo can be divided into steps including downloading, building, installing, making Occlum instance, running Occlum instance, and checking result. Contributors could write simple script or use build system like make or cmake.


## Step 3: Add New Demo to CI

It is the last one step before submitting but very important, which can definitely boost the speed of review and merge. And it also makes sure that this demo is tested for every PR, commit and release. So that it won't get broken in a new version.

### 3.1 - Simple dependency

For demos with simple dependency, contributors can refer to [this file](https://github.com/occlum/occlum/blob/master/.github/workflows/demo_test.yml) to add a job in parallel. However, if the job lasts over `25 min` in a single run, it is recommended to choose the second path listed below.

### 3.2 - Complex dependency

For demos with complex dependency, it is better to create a docker image for that. Contributors could test this in their own repository. Let's take [OpenVino demo](https://github.com/occlum/occlum/tree/master/demos/openvino) as an example.

#### 3.2.1 - Create dockerfile for the demo

Just use Occlum image as the base image. And download, build and install all the dependencies in the image. This [dockerfile](https://github.com/occlum/occlum/blob/master/tools/docker/ci/Dockerfile.openvino) can be used as a reference.

#### 3.2.2 - Define steps for building image

After dockerfile is provided, contributors should also define the steps to build the image. [Here](https://github.com/occlum/occlum/blob/a9574ca22e5684413a0278591b811ac538ce3c17/.github/workflows/build_and_push_ci_image.yml#L102) is a reference for this step.

#### 3.2.3 - Define test steps for the demo

After the image is ready, the demo should be added as a single job. The steps are easy. Pull the image, install Occlum and run demo test. Take [this](https://github.com/occlum/occlum/blob/a9574ca22e5684413a0278591b811ac538ce3c17/.github/workflows/demo_test.yml#L341) as a reference.


## Step 4: Wait for Review and Merge

OK! Finally, this is good to go. We appreciate your time and your delicate work. Thank you for your contributions! All you need to do next is just waiting and maybe some modifications according to the review comments.

We are looking forward to your contributions! Thank you.
