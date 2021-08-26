# Quick Start: running Occlum with OCI runtime rune

This user guide provides the steps to run Occlum with OCI Runtime `rune`.

[rune](https://github.com/alibaba/inclavare-containers/tree/master/rune) is a novel OCI Runtime used to run trusted applications in containers with the hardware-assisted enclave technology.

[Occlum](https://github.com/occlum/occlum) is a memory-safe, multi-process library OS for Intel SGX.

# Requirements

- Ensure that you have one of the following required operating systems to build an Occlum container image:
  - CentOS 8.2
  - Ubuntu 18.04-server

- Please follow [Intel SGX Installation Guide](https://download.01.org/intel-sgx/sgx-linux/2.13/docs/Intel_SGX_Installation_Guide_Linux_2.13_Open_Source.pdf) to install Intel SGX driver, Intel SGX SDK & PSW for Linux.
  - For CentOS 8.2, UAE service libraries are needed but may not be installed if SGX PSW installer is used. Please manually install it:
    ```shell
    yum install libsgx-uae-service
    ```

- Install [enable_rdfsbase kernel module](https://github.com/occlum/enable_rdfsbase#how-to-build), allowing to use FSGSBASE instructions in Occlum. Please skip this step when using kernel 5.9. Note that you are not able to run Occlum with kernel disabled FSGSBASE feature even you have installed this module.

- Install rune and occlum.
  - For CentOS 8.2:
    1. Add the repository to your sources.
    ```shell
    cat >/etc/yum.repos.d/inclavare-containers.repo <<EOF
    [inclavare-containers]
    name=inclavare-containers
    enabled=1
    baseurl=https://mirrors.openanolis.org/inclavare-containers/rpm-repo/
    gpgcheck=1
    repo_gpgcheck=1
    gpgkey=https://mirrors.openanolis.org/inclavare-containers/rpm-repo/RPM-GPG-KEY-rpm-sign
    gpgcakey=https://mirrors.openanolis.org/inclavare-containers/rpm-repo/RPM-GPG-KEY-rpm-sign-ca
    EOF
    ```

    2. Install the RPM packages.
    ```shell
    sudo yum install -y rune occlum
    source /etc/profile
    ```

  - For Ubuntu 18.04-server:
    1. Add the repository to your sources.
    ```shell
    echo 'deb [arch=amd64] https://mirrors.openanolis.org/inclavare-containers/deb-repo bionic main' | tee /etc/apt/sources.list.d/inclavare-containers.list
    ```

    2. Add the key to the list of trusted keys used by the apt to authenticate packages.
    ```shell
    wget -qO - https://mirrors.openanolis.org/inclavare-containers/deb-repo/DEB-GPG-KEY.key | sudo apt-key add -
    ```

    3. Update the apt and install the packages.
    ```shell
    sudo apt-get update
    sudo apt-get install -y rune occlum
    source /etc/profile
    ```

# Building Occlum container image

## Prepare "hello world" demo program

[This tutorial](https://github.com/occlum/occlum#hello-occlum) can help you to create your first occlum build.

Assuming the "hello world" demo program in `occlum_instance` directory is built.

Type the following commands to generate a minimal, self-contained package (.tar.gz) for the Occlum instance.

```shell
cd occlum_instance
occlum package occlum_instance.tar.gz
```

## Create Occlum container image

Now you can build your occlum container image in `occlum_instance` directory on your host system.

Type the following commands to create a `Dockerfile`:

```Dockerfile
cat >Dockerfile <<EOF
FROM centos:8.2.2004

RUN mkdir -p /run/rune
WORKDIR /run/rune

ADD occlum_instance.tar.gz /run/rune

ENTRYPOINT ["/bin/hello_world"]
EOF
```

then build the Occlum container image with the command:

```shell
docker build . -t occlum-app
```

# Configuring OCI Runtime rune for Docker

Add the associated configuration for `rune` in dockerd config file, e.g, `/etc/docker/daemon.json`, on your system.

```json
{
	"runtimes": {
		"rune": {
			"path": "/usr/local/bin/rune",
			"runtimeArgs": []
		}
	}
}
```

then restart dockerd on your system.

You can check whether `rune` is correctly enabled or not with:

```shell
docker info | grep rune
```

The expected result would be:

```
Runtimes: rune runc
```

# Running Occlum container image

You need to specify a set of parameters to `docker run` to run:

```shell
docker run -it --rm --runtime=rune \
  -e ENCLAVE_TYPE=intelSgx \
  -e ENCLAVE_RUNTIME_PATH=/opt/occlum/build/lib/libocclum-pal.so \
  -e ENCLAVE_RUNTIME_ARGS=occlum_instance \
  occlum-app
```

where:
- @ENCLAVE_TYPE: specify the type of enclave hardware to use, such as `intelSgx`.
- @ENCLAVE_PATH: specify the path to enclave runtime PAL to launch.
- @ENCLAVE_ARGS: specify the specific arguments to enclave runtime PAL, separated by the comma.

# Deployment

Please refer to this [guide](https://www.alibabacloud.com/help/doc-detail/254909.htm) to show how to deploy confidential containers in TEE-based ACK clusters and this [guide](https://www.alibabacloud.com/help/doc-detail/259685.htm) to show how to use confidential containers to implement remote attestation in TEE-based ACK clusters.
