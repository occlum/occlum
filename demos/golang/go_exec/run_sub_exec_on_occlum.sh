#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

# 1. Compile helloworld.go and exec.go
occlum-go build exec.go
occlum-go build helloworld.go

# 2. Init Occlum Workspace
rm -rf occlum_instance && mkdir occlum_instance
cd occlum_instance
occlum init
new_json="$(jq '.resource_limits.user_space_size = "1MB" |
                .resource_limits.user_space_max_size = "2000MB" ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 3. Copy program into Occlum Workspace and build
rm -rf image && \
copy_bom -f ../sub_exec.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# 4. Run the sub process exec sample
echo -e "${BLUE}occlum run /bin/exec${NC}"
occlum run /bin/exec
