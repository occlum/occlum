# Dragonwell11 For Enclave

Dragonwell11 for enclave is a musl-based JDK version compatible with the Alpine Linux and Occlum, and it's an open source project, see [here](https://github.com/alibaba/dragonwell11/tree/dragonwell-for-enclave) for the code.

Dragonwell11 for enclave must be built in the Alpine Docker container, you can change to the top path of Dragonwell11 project, then run the `./make.sh release` command to build it.

## How to install

We provide three ways to install the Dragonwell11 JDK image:

1. Download Dragonwell11 JDK image directly from the OSS, then unzip and copy it into the host path `/opt/occlum/toolchains/jvm`.
    ```
    wget https://dragonwell.oss-cn-shanghai.aliyuncs.com/11/linux/x64/11.0.8.3-enclave/Dragonwell11-11.0.8.3.tar.gz
    ```

2. Download Dragonwell11 `src.rpm` from OSS, the `src.rpm` will build dragonwell11 project and then install the image automatically. The `src.rpm` contains the source code and spec file of Dragonwell11, it will build Dragonwell11 and then copy the built image into host path `/opt/occlum/toolchains/jvm`. Be cautious that Docker container should not work in root mode.
    ```
    wget https://dragonwell.oss-cn-shanghai.aliyuncs.com/11/linux/x64/11.0.8.3-enclave/Dragonwell11-11.0.8.3-EnclaveExperimental.src.rpm rpm -ivh dragonwell11-for-enclave-11.0.8.3-EnclaveExperimental.src.rpm --nodeps --force
    ```

3. Download Dragonwell11 rpm file from OSS, the rpm file contains the JDK image and spec file of Dragonwell11. It will not build the Dragonwell11 from source code, but install the JDK image directly, which will copy the Dragonwell11 image into host path `/opt/occlum/toolchains/jvm`.
    ```
    wget https://dragonwell.oss-cn-shanghai.aliyuncs.com/11/linux/x64/11.0.8.3-enclave/Dragonwell11-11.0.8.3-EnclaveExperimental.x86_64.rpm rpm -ivh dragonwell11-for-enclave-11.0.8.3-EnclaveExperimental.x86_64.rpm --nodeps --force
    ```
