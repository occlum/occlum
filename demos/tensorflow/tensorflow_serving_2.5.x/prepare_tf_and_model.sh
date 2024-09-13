#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

container="tensorflow/serving"
tag="2.5.1"
dest="${script_dir}/tf_serving"

rm -f rootfs.tar
docker export $(docker create --network host --name rootfs_dump ${container}:${tag}) -o rootfs.tar
docker rm rootfs_dump

rm -rf ${dest}/rootfs && mkdir -p ${dest}/rootfs
tar xf rootfs.tar -C ${dest}/rootfs
rm -f rootfs.tar

echo "Successfully dumped ${container}:${tag} rootfs to ${dest}/rootfs."

pushd $dest
# Download pretrained resnet model
rm -rf resnet*
wget https://tfhub.dev/tensorflow/resnet_50/classification/1?tf-hub-format=compressed -O resnet.tar.gz
mkdir -p resnet/123
tar zxf resnet.tar.gz -C resnet/123
popd
