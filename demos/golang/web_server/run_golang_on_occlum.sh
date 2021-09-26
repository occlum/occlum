#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

web_server="./web_server"

if [ ! -f $web_server ];then
    echo "Error: cannot stat file '$web_server'"
    echo "Please see README and build it using Occlum Golang toolchain"
    exit 1
fi

# 1. Init Occlum Workspace
rm -rf occlum_instance && mkdir occlum_instance
cd occlum_instance
occlum init
new_json="$(jq '.resource_limits.user_space_size = "1000MB" |
                .process.default_mmap_size = "900MB"' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy program into Occlum Workspace and build
rm -rf image && \
copy_bom -f ../web_server.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# 3. Run the web server sample
echo -e "${BLUE}occlum run /bin/web_server${NC}"
occlum run /bin/web_server
