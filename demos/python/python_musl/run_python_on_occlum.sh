#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

alpine_fs="/root/alpine_python"

if [ ! -d $alpine_fs ];then
    echo "Error: cannot stat '$alpine_fs' directory"
    exit 1
fi

# 1. Init Occlum Workspace
[ -d occlum_instance ] || mkdir occlum_instance
cd occlum_instance
[ -d image ] || occlum init

# 2. Copy files into Occlum Workspace and build
if [ ! -d "image/lib/python3.7" ];then
    rm -rf image
    copy_bom -f ../python_musl.yaml --root image --include-dir /opt/occlum/etc/template
    yq '.resource_limits.user_space_size.init = "320MB" |
        .resource_limits.kernel_space_heap_size.init = "512MB" |
        .mount += [{"target": "/host", "type": "hostfs", "source": "."}] ' -i Occlum.yaml

    occlum build
fi

# 3. Run the hello world sample
echo -e "${BLUE}occlum run /bin/python3.7 demo.py${NC}"
occlum run /bin/python3.7 demo.py
