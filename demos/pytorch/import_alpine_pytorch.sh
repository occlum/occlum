#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

alpine_container=$1
target_container=$2

if [ "$alpine_container" == "" -o "$target_container" == "" ];then
cat <<EOF
Import the rootfs of Alpine Linux's Pytorch Docker image into a target Occlum container (/root/alpine_pytorch)

USAGE:
    ./import_alpine_pytorch.sh <alpine_container> <target_container>

<alpine_container>:
    The id or name of Alpine Linux Docker container.

<target_container>:
    The id or name of Docker container that you want to copy to.
EOF
    exit 1
fi

alpine_pytorch_tar="$script_dir/alpine_pytorch.tar"
alpine_pytorch="$script_dir/alpine_pytorch"

# Export the rootfs from Alpine's Docker image
docker export -o $alpine_pytorch_tar $alpine_container

# Copy the exported rootfs to the Occlum container
rm -rf $alpine_pytorch && mkdir -p $alpine_pytorch
tar -xf $alpine_pytorch_tar -C $alpine_pytorch
docker cp $alpine_pytorch $target_container:/root/

# Clean up
rm -rf $alpine_pytorch $alpine_pytorch_tar
