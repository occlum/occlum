#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

occlum_pong="./occlum_pong"

if [ ! -f $occlum_pong ];then
    echo "Error: cannot stat file '$occlum_pong'"
    echo "Please see README and build it using prepare_pong_pong.sh"
    exit 1
fi

# Init Occlum Workspace
rm -rf occlum_pong_instance && mkdir occlum_pong_instance
cd occlum_pong_instance
occlum init
new_json="$(jq '.resource_limits.user_space_size = "1MB" |
	.resource_limits.user_space_max_size = "1000MB" |
	.resource_limits.kernel_space_heap_size="1MB" |
	.resource_limits.kernel_space_heap_max_size="80MB" ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy program into Occlum Workspace and build
rm -rf image && \
copy_bom -f ../pong.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# 3. Run the hello world sample
echo -e "${BLUE}occlum run /bin/occlum_pong${NC}"
time occlum run /bin/occlum_pong
