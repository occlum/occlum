# Quick Start: rune on Occlum

[rune](https://github.com/alibaba/inclavare-containers) is a set of tools for running trusted applications in containers with the hardware-assisted enclave technology.

## Hardware requirements
- Install [Intel SGX driver for Linux](https://github.com/intel/linux-sgx-driver#build-and-install-the-intelr-sgx-driver), required by Intel SGX SDK && PSW.
- Install [enable_rdfsbase kernel module](https://github.com/occlum/enable_rdfsbase#how-to-build), allowing to use `rdfsbase` -family instructions in Occlum.

---

## Build and install rune
`rune` is a CLI tool for spawning and running enclaves in containers according to the OCI specification.

Please refer to [this guide](https://github.com/alibaba/inclavare-containers#rune) to build `rune` from scratch.

---

## Build Occlum application bundle
### Download Occlum sdk image
``` shell
yum install -y libseccomp-devel
mkdir "$HOME/rune_workdir"
docker pull occlum/occlum:0.12.0-centos7.5
docker run -it --device /dev/isgx \
  -v $HOME/rune_workdir:/root/rune_workdir \
  occlum/occlum:0.12.0-centos7.5
```

### Prepare the materials
Before Occlum build, execute the following command to set your Occlum instance dir:
``` shell
export OCCLUM_INSTANCE_DIR=occlum-app
```
You can build a "hello world" demo application or your own product with the [Occlum CentOS Docker image](https://hub.docker.com/r/occlum/occlum/tags).

[This guide](https://github.com/occlum/occlum#hello-occlum) can help you to create your first occlum build.

After Occlum build, execute the following commands in Occlum sdk container environment:

``` shell
yum install -y libseccomp-devel
cp -a occlum-app /root/rune_workdir
cd /root/rune_workdir
mkdir lib
cp /usr/lib64/libseccomp.so.2 lib
cp /usr/lib64/libprotobuf.so.* lib
cp /usr/lib64/libsgx_u*.so* lib
cp /usr/lib64/libsgx_enclave_common.so.1 lib
cp /usr/lib64/libsgx_launch.so.1 lib
```

### Build occlum application image
Now you can build your occlum application image in the `$HOME/rune_workdir` directory of your host system.

Type the following commands to create a `Dockerfile`:
``` Dockerfile
cat >Dockerfile <<EOF
FROM centos:7.5.1804

RUN mkdir -p /run/rune/occlum-app
WORKDIR /run/rune

COPY lib /lib
COPY occlum-app occlum-app

RUN ln -sfn occlum-app/build/lib/libocclum-pal.so liberpal-occlum.so
RUN ldconfig

ENTRYPOINT ["/bin/hello_world"]
EOF
```

and then build it with the command:
```shell
docker build . -t occlum-app
```

### Create bundle
In order to use `rune` you must have your container in the format of an OCI bundle. If you have Docker installed you can use its `export` method to acquire a root filesystem from an existing Docker container.

``` shell
# create the top most bundle directory
cd "$HOME/rune_workdir"
mkdir rune-container
cd rune-container

# create the rootfs directory
mkdir rootfs

# export occlum-app via Docker into the rootfs directory
docker export $(docker create occlum-app) | sudo tar -C rootfs -xvf -
```

After a root filesystem is populated you just generate a spec in the format of a config.json file inside your bundle. `rune` provides a spec command which is similar to `runc` to generate a template file that you are then able to edit.

``` shell
rune spec
```

To find features and documentation for fields in the spec please refer to the [specs](https://github.com/opencontainers/runtime-spec) repository.

In order to run the hello world demo program in Occlum with `rune`, you need to change the entrypoint from `sh` to `/bin/hello_world`
``` json
  "process": {
      "args": [
          "/bin/hello_world"
      ],
  }
```

and then configure enclave runtime as following:
``` json
  "annotations": {
      "enclave.type": "intelSgx",
      "enclave.runtime.path": "/run/rune/liberpal-occlum.so",
      "enclave.runtime.args": "occlum-app"
  }
```

---

## Run Occlum application
Assuming you have an OCI bundle from the previous step you can execute the container in this way.

``` shell
cd "$HOME/rune_workdir/rune-container"
sudo rune run rune-container
```
