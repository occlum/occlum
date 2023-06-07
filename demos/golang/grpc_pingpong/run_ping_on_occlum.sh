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
yq '.resource_limits.user_space_size.init = "800MB" |
	.resource_limits.kernel_space_heap_size.init ="512MB" |
	.resource_limits.kernel_space_stack_size="1MB" |
	.process.default_stack_size = "1MB" |
	.process.default_heap_size = "20MB" ' -i Occlum.yaml

# 2. Copy program into Occlum Workspace and build
rm -rf image && \
copy_bom -f ../ping.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# 3. Run the hello world sample
echo -e "${BLUE}occlum run /bin/occlum_ping${NC}"
time occlum run /bin/occlum_ping
