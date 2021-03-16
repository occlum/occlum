# Occlum Installer

To evaluate Occlum in non-docker environment, installers are needed. Occlum provides a variety of installers to support different scenarios. Users can choose to install different minimum subsets of Occlum installers to meet their needs.

- **occlum-runtime**: necessary binaries for `occlum run/exec`. For deployment scenarios, this must be installed.
- **occlum-pal**: only contains the Occlum PAL library (a thin layer to hide details of libOS and provide API for applications)
- **occlum-sgx-tools**: minimum dependencies from Intel SGX SDK e.g. sgx-gdb, sgx_sign
- **occlum-toolchains-\<language\>**: toolchain components for specific language
- **occlum**: complete package to support all Occlum commands. `occlum-toolchains-gcc` is also installed by default. Please install packages of other programming languages based on your need.

## RPM Installer

### How to Build

Normally, Occlum installers should be provided together with release. However, users can also build them on their own.

To build RPM packages, a docker container with Occlum CentOS image (based on CentOS 8.2) is needed. Execute below commands under the occlum directory:
```
cd tools/installer/rpm
make
```
and occlum rpm installer can be found under `build/rpms`.

If a user wants to build his application on a platform installed with Occlum installer, toolchain installers are also needed. To build language specific toolchain installer, just run the command:
```
cd tools/installer/rpm
make <language option>
```
Now, only `musl-gcc` and `golang` options are supported. And the installer can be found under `build/rpms`.

### How to Use

RPM installer should be found together with Occlum release package at [this page](https://github.com/occlum/occlum/releases).
To run Occlum on clean Centos 8, please follow below steps:

**Step 1. Install Prerequisites**
```
yum install -y libcurl-devel openssl-devel fuse-devel fuse-libs autoconf automake cmake libtool make yum-utils gdb python2
ln -s /usr/bin/python2 /usr/local/bin/python
dnf config-manager --set-enabled PowerTools
yum install -y ocaml ocaml-ocamlbuild
```

**Step 2. Install Intel® SGX driver and Intel® SGX PSW**
Please follow [Intel SGX Installation Guide](https://download.01.org/intel-sgx/sgx-linux/2.13/docs/Intel_SGX_Installation_Guide_Linux_2.13_Open_Source.pdf) to install SGX driver and SGX PSW. SGX SDK is not required. Using RPM installer is recommanded.

Also, UAE service libraries are needed but may not installed together with SGX PSW if SGX PSW installer is used. Go to SGX RPM local repo and run:
```
rpm -i libsgx-uae-service-*.rpm
```

**Step 3. Install enable_RDFSBASE Kernel Module**
Please follow [this README](https://github.com/occlum/enable_rdfsbase/blob/master/README.md) to install `enable_rdfsbase` kernel module.

**Step 4. Install Occlum Installer and Toolchains Installer**
```
rpm -i occlum-sgx-tools-*.rpm
rpm -i occlum-pal-*.rpm
rpm -i occlum-runtime-*.rpm
```

Toolchains are needed when compiling applications and also during runtime. C/C++ toolchain is a must for Occlum commands.
To install C/C++ toolchain, just run the command:
```
rpm -i occlum-toolchains-gcc-*.rpm
```

Besides, users can choose to install the toolchain installer based on the application's language. Currently, we also supports Golang. More language toolchain installers are on the way. To install Golang toolchain, run the below commands:
```
yum install -y epel-release
yum install -y rc
rpm -i occlum-toolchains-golang-*.rpm
```

At last, install `occlum` package to get complete support of Occlum:
```
rpm -i occlum_*.rpm
```

To make the new installed binaries and libraries work, this command must be executed:
```
source /etc/profile
```

**Step 5. Install Debug Packages (OPTIONAL)**
If users want to debug the application running inside the libos, debug packages are also needed. Just run:
```
rpm -i occlum-debuginfo*.rpm occlum-debugsource*.rpm occlum-pal-debuginfo*.rpm occlum-runtime-debuginfo*.rpm occlum-sgx-tools-debuginfo*.rpm occlum-toolchains-gcc-debuginfo*.rpm occlum-toolchains-gcc-debugsource*.rpm
```


## DEB Installer

### How to Build

Normally, Occlum installers should be provided together with release. However, users can also build them on their own.

To build deb packages, a docker container with Occlum Ubuntu image (based on Ubuntu 18.04) is needed. Execute below commands under the occlum directory:
```
cd tools/installer/deb
make
```
and occlum deb installer can be found under `build/debs`.

If a user wants to build his application on a platform installed with Occlum installer, toolchain installers are also needed. To build language specific toolchain installer, just run the command:
```
cd tools/installer/deb
make <language option>
```
Now, only `musl-gcc` and `golang` options are supported. And the installer can be found under `build/debs`.

### How to Use

DEB installer should be found together with Occlum release package at [this page](https://github.com/occlum/occlum/releases).
To run Occlum on clean Ubuntu 18.04, please follow below steps:

**Step 1. Install Prerequisites**
```
apt-get update
apt-get install -y --no-install-recommends libcurl4-openssl-dev libssl-dev libprotobuf-dev libfuse-dev autoconf automake make cmake libtool gdb python jq ca-certificates gnupg wget vim
```

**Step 2. Install Intel® SGX driver and Intel® SGX PSW**
Please follow [Intel SGX Installation Guide](https://download.01.org/intel-sgx/sgx-linux/2.13/docs/Intel_SGX_Installation_Guide_Linux_2.13_Open_Source.pdf) to install SGX driver and SGX PSW. SGX SDK is not required. Using PSW installer is recommanded.

To install PSW, follow the guide to add Intel® SGX repository to apt source. And then run:
```
apt-get update
apt-get install -y libsgx-epid libsgx-urts libsgx-quote-ex libsgx-uae-service
```

After installing PSW, please make sure that the aesm service is running:
```
service aesmd status
```

**Step 3. Install enable_RDFSBASE Kernel Module**
Please follow [this README](https://github.com/occlum/enable_rdfsbase/blob/master/README.md) to install `enable_rdfsbase` kernel module.

**Step 4. Install Occlum Installer and Toolchains Installer**
```
cd <path to installer>
apt install -y ./occlum-runtime*.deb
apt install -y ./occlum-pal*.deb
apt install -y ./occlum-sgx-tools*.deb
```

Toolchains are needed when compiling applications and also during runtime. C/C++ toolchain is a must for Occlum commands.
To install C/C++ toolchain, just run the command:
```
apt install -y ./occlum-toolchains-gcc*.deb
```

Besides, users can choose to install the toolchain installer based on the application's language. Currently, we also supports Golang. More language toolchain installers are on the way. To install Golang toolchain, run the below commands:
```
apt install -y ./occlum-toolchains-golang*.deb
```

At last, install `occlum` package to get complete support of Occlum:
```
apt install -y ./occlum_*.deb
```

To make the new installed binaries and libraries work, this command must be executed:
```
source /etc/profile
```

**Step 5. Install Debug Symbol Packages (OPTIONAL)**
If users want to debug the application running inside the libos, debug symbol packages are also needed. Just run:
```
apt install -y ./occlum-dbgsym*.ddeb ./occlum-pal-dbgsym*.ddeb ./occlum-runtime-dbgsym*.ddeb ./occlum-toolchains-gcc-dbgsym*.ddeb ./occlum-sgx-tools-dbgsym*.ddeb
```
