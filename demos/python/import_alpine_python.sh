#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

alpine_container=$1
target_container=$2

if [ "$alpine_container" == "" -o "$target_container" == "" ];then
cat <<EOF
Import the rootfs of Alpine Linux's Python Docker image into a target Occlum container (/root/alpine_python)

USAGE:
    ./import_alpine_python.sh <alpine_container> <target_container>

<alpine_container>:
    The id or name of Alpine Linux Docker container.

<target_container>:
    The id or name of Docker container that you want to copy to.
EOF
    exit 1
fi

alpine_python_tar="$script_dir/alpine_python.tar"
alpine_python="$script_dir/alpine_python"

# Export the rootfs from Alpine's Docker image
docker export -o $alpine_python_tar $alpine_container

# Copy the exported rootfs to the Occlum container
rm -rf $alpine_python && mkdir -p $alpine_python
tar -xf $alpine_python_tar -C $alpine_python
docker cp $alpine_python $target_container:/root/

# Clean up
rm -rf $alpine_python $alpine_python_tar
