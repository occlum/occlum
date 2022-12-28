# Install Occlum with Popular Package Managers

Occlum can be easily installed with popular package managers like [APT](https://en.wikipedia.org/wiki/APT_(software)) and [RPM](https://en.wikipedia.org/wiki/RPM_Package_Manager). This document walks you through the steps to install Occlum on popular Linux distributions like Ubuntu and CentOS using package managers.

## Prerequisite

**Install enable_RDFSBASE Kernel Module**

If the Kernel version is before `v5.9`, please follow [this README](https://github.com/occlum/enable_rdfsbase/blob/master/README.md) to install `enable_rdfsbase` kernel module.


## Install Occlum with RPM on CentOS 8.2

1. Install Prerequisite
```
yum install -y wget yum-utils make jq gdb
```

2. Install Intel® SGX Driver and Intel® SGX PSW

Please follow [Intel SGX Installation Guide](https://download.01.org/intel-sgx/sgx-linux/2.13/docs/Intel_SGX_Installation_Guide_Linux_2.13_Open_Source.pdf) to install SGX driver and SGX PSW. SGX SDK is not required. Using RPM installer is recommanded. 

After adding SGX RPM local repository to yum source, run the below command to install PSW:
```
yum --nogpgcheck install -y libsgx-dcap-ql libsgx-epid libsgx-urts libsgx-quote-ex libsgx-dcap-quote-verify-dev
```

Also, UAE service libraries are needed but may not installed together with SGX PSW if SGX PSW installer is used. Go to SGX RPM local repo and run:
```
rpm -i libsgx-uae-service*.rpm
```

After installing SGX driver and PSW, please make sure `aesm` service is in `active (running)` state by checking:
```
service aesmd status
```

3. Install Occlum
```
cat << EOF > /etc/yum.repos.d/occlum.repo
[occlum]
name=occlum
enabled=1
baseurl=https://occlum.io/occlum-package-repos/rpm-repo/
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://occlum.io/occlum-package-repos/rpm-repo/RPM-GPG-KEY-rpm-sign
gpgcakey=https://occlum.io/occlum-package-repos/rpm-repo/RPM-GPG-KEY-rpm-sign-ca
EOF
yum --showduplicate list -y occlum
yum install -y occlum
echo "source /etc/profile" >> $HOME/.bashrc
```


## Install Occlum with APT on Ubuntu 18.04

1. Install Prerequisite
```
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates gnupg2 jq make gdb wget libfuse-dev libtool tzdata
```

2. Install Intel® SGX Driver and Intel® SGX PSW

Please follow [Intel SGX Installation Guide](https://download.01.org/intel-sgx/sgx-linux/2.13/docs/Intel_SGX_Installation_Guide_Linux_2.13_Open_Source.pdf) to install SGX driver and SGX PSW. SGX SDK is not required. Using PSW installer is recommended.

To install PSW, follow the guide to add Intel® SGX repository to APT source. And then run:
```
apt-get update
apt-get install -y libsgx-dcap-ql libsgx-epid libsgx-urts libsgx-quote-ex libsgx-uae-service libsgx-dcap-quote-verify-dev
```

After installing PSW, please make sure `aesm` service is in `active (running)` state by checking:
```
service aesmd status
```

3. Install Occlum
```
echo 'deb [arch=amd64] https://occlum.io/occlum-package-repos/debian bionic main' | tee /etc/apt/sources.list.d/occlum.list
wget -qO - https://occlum.io/occlum-package-repos/debian/public.key | apt-key add -
apt-get update
apt-get install -y occlum
echo "source /etc/profile" >> $HOME/.bashrc
```


## Hello World Test

You are all set. Now, let's run a hello world test:
```
cd /tmp && wget https://raw.githubusercontent.com/occlum/occlum/master/demos/hello_c/hello_world.c
occlum-gcc -o hello_world hello_world.c
occlum new occlum-instance
cp hello_world /tmp/occlum-instance/image/bin
cd /tmp/occlum-instance && occlum build
occlum run /bin/hello_world
```


## Version Compatibility Matrix

When version is not specified, Occlum with the latest version will be installed. If a user would like to evaluate an older version, please make sure the corresponding Intel® SGX PSW is installed.

The matrix below shows the version compatibility since Occlum `0.16.0`. Please check before installing or upgrading.

| Occlum Version  |  SGX PSW Version  | Tested under Ubuntu | Tested under CentOS |
| --------------- | ----------------- | ------------------- | ------------------- |
|     0.16.0      |       2.11        |        18.04        |         8.1         |
|     0.17.0      |       2.11        |        18.04        |         8.1         |
|     0.18.1      |       2.11        |        18.04        |         8.1         |
|     0.19.1      |       2.11        |        18.04        |         8.1         |
|     0.20.0      |       2.11        |        18.04        |         8.1         |
|     0.21.0      |       2.13        |        18.04        |         8.2         |
|     0.23.1      |       2.13.3      |        18.04        |         8.2         |
|     0.24.1      |       2.14        |        18.04        |         8.2         |

For more information about the packages, please checkout [here](../tools/installer/README.md).
