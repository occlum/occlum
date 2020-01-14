#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

target_container=$1

if [ "$target_container" == "" ];then
cat <<EOF
Import the rootfs of Alpine Linux's Python Docker image into a target Occlum container (/root/alpine_python)

USAGE:
    ./import_alpine_python.sh <target_container>

<target_container>:
    The id or name of Docker container that you want to copy to.
EOF
    exit 1
fi

alpine_python_container="alpine_python_docker"
alpine_python_tar="$script_dir/alpine_python.tar"
alpine_python="$script_dir/alpine_python"

# Export the rootfs from Alpine's Docker image
docker pull python:3.7-alpine3.10
docker create --name $alpine_python_container python:3.7-alpine3.10
docker export -o $alpine_python_tar $alpine_python_container
docker rm $alpine_python_container

# Copy the exported rootfs to the Occlum container
rm -rf $alpine_python && mkdir -p $alpine_python
tar -xf $alpine_python_tar -C $alpine_python
docker cp $alpine_python $target_container:/root/

# Clean up
rm -rf $alpine_python $alpine_python_tar
