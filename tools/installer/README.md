# Occlum Installer

## RPM Installer

### How to Build

To build RPM packages, a docker container with Occlum CentOS image is needed. Execute below commands under the occlum directory:
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
Now, only `c/c++` option is supported. And the installer can be found under `build/rpms`.

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
Please follow [Intel SGX Installation Guide](https://download.01.org/intel-sgx/sgx-linux/2.9.1/docs/Intel_SGX_Installation_Guide_Linux_2.9.1_Open_Source.pdf) to install SGX driver and SGX PSW. SGX SDK is not required. Using RPM installer is recommanded.

Also, UAE service libraries are needed but may not installed together with SGX PSW if SGX PSW installer is used. Go to SGX RPM local repo and run:
```
rpm -i libsgx-uae-service-2.9.101.2-1.el7.x86_64.rpm
```

**Step 3. Install Occlum Installer and Toolchains Installer**
```
rpm -i occlum-sgx-tools-*.rpm
rpm -i occlum-"$occlum_version"-*.rpm
rpm -i occlum-pal-*.rpm
rpm -i occlum-platform-*.rpm
```

Toolchains are needed when compile applications and also during runtime. Choose to install the toolchain installer based on the application's language. Currently, we only supports `C/C++`. More language toolchain installers are on the way. To install `C/C++` toolchain, just run the command:
```
rpm -i occlum-toolchains-gcc-*.rpm
```

To make the new installed binaries and libraries work, this command must be executed:
```
source /etc/profile
```

Finally, you are good to go!

### Build DEB Installer
TBD
