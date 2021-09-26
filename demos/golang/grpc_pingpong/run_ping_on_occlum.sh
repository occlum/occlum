#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

occlum_ping="./occlum_ping"

if [ ! -f $occlum_ping ];then
    echo "Error: cannot stat file '$occlum_ping'"
    echo "Please see README and build it using prepare_ping_pong.sh"
    exit 1
fi

# Init Occlum Workspace
rm -rf occlum_ping_instance && mkdir occlum_ping_instance
cd occlum_ping_instance
occlum init
new_json="$(jq '.resource_limits.user_space_size = "2560MB" |
	.resource_limits.kernel_space_heap_size="320MB" |
	.resource_limits.kernel_space_stack_size="10MB" |
	.process.default_stack_size = "40MB" |
	.process.default_heap_size = "320MB" |
	.process.default_mmap_size = "960MB" ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy program into Occlum Workspace and build
rm -rf image && \
copy_bom -f ../ping.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# 3. Run the hello world sample
echo -e "${BLUE}occlum run /bin/occlum_ping${NC}"
time occlum run /bin/occlum_ping
