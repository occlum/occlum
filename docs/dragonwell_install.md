# Dragonwell11 For Enclave

Dragonwell11 for enclave is an opensource project, see [here](https://github.com/alibaba/dragonwell11/tree/dragonwell-for-enclave).

It's a musl-based JDK version compatible with Alpine platform and Occlum libos. Dragonwell11 for enclave will be built in a Alpine docker,

you can cd the top path of dragonwell11 project, will see make.sh and musl.sh files, then './make.sh release' begin to built dragonwell11

for enclave.

# How to install Dragonwell11

We provide three ways to install dragonwell11 jdk image:

1. download dragonwell11 jdk image directly from oss, then decompress and copy it into host path /opt/occlum/toolchains/jvm

   wget https://dragonwell.oss-cn-shanghai.aliyuncs.com/11/linux/x64/11.0.8.3-enclave/Dragonwell11-11.0.8.3.tar.gz

2. download dragonwell11 src.rpm from oss, the src.rpm will build dragonwell11 project and then install the image automatically.

   src.rpm contains dragonwell11 source code and spec file, it will build dragonwell11 and then copy the built image into

   host path /opt/occlum/toolchains/jvm. Be cautious that docker should not work in root mode.

   wget https://dragonwell.oss-cn-shanghai.aliyuncs.com/11/linux/x64/11.0.8.3-enclave/Dragonwell11-11.0.8.3-EnclaveExperimental.src.rpm
   rpm -ivh dragonwell11-for-enclave-11.0.8.3-EnclaveExperimental.src.rpm --nodeps --force

3. download dragonwell11 rpm from oss, the .rpm contains dragonwell11 jdk image and spec file. it will not build the dragonwell11

   source code, but install the jdk image directly, which will copy the dragonwell11 image into host path /opt/occlum/toolchains/jvm.

   wget https://dragonwell.oss-cn-shanghai.aliyuncs.com/11/linux/x64/11.0.8.3-enclave/Dragonwell11-11.0.8.3-EnclaveExperimental.x86_64.rpm
   rpm -ivh dragonwell11-for-enclave-11.0.8.3-EnclaveExperimental.x86_64.rpm --nodeps --force
