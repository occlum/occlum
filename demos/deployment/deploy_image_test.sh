#! /bin/bash
set -xe

if [[ $1 != "ubuntu18.04" && $1 != "centos8.2" ]]; then
    echo "Must choose between <ubuntu18.04/centos8.2>."
    exit 1
fi

OS=$1
if [ -z "$DEVICE_OPTION" ]; then
    DEVICE_OPTION="--device /dev/isgx"
fi

docker build -f Dockerfile_template."$OS" -t test-package:"$OS" .
name="$OS"_deploy_test

docker rm -f $name || true
docker run --name="$name" --hostname="$name" --net="host" --privileged $DEVICE_OPTION test-package:"$OS" bash -c "source /root/.bashrc; cd /root/occlum-instance; occlum run /bin/hello_world"
