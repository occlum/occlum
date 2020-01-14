#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

alpine_fs="/root/alpine_python"

if [ ! -d $alpine_fs ];then
    echo "Error: cannot stat '$alpine_fs' directory"
    echo "Please see README and import the rootfs of Alpine Linux's Python Docker image"
    exit 1
fi

# 1. Init Occlum Workspace
rm -rf occlum_context && mkdir occlum_context
cd occlum_context
occlum init

# 2. Copy files into Occlum Workspace and build
cp $alpine_fs/usr/local/bin/python3.7 image/bin
cp $alpine_fs/usr/local/lib/libpython3.7m.so.1.0 image/lib
cp $alpine_fs/usr/local/lib/libpython3.so image/lib
cp -r $alpine_fs/usr/local/lib/python3.7 image/lib
cp ../hello.py image
occlum build

# 3. Run the hello world sample
echo -e "${BLUE}occlum run /bin/python3.7 hello.py${NC}"
occlum run /bin/python3.7 hello.py
