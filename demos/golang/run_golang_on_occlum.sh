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
rm -rf occlum_context && mkdir occlum_context
cd occlum_context
occlum init
jq '.resource_limits.user_space_size = "380MB"' Occlum.json > temp_Occlum.json
jq '.process.default_mmap_size = "300MB"' temp_Occlum.json > Occlum.json

# 2. Copy program into Occlum Workspace and build
cp ../web_server image/bin
occlum build

# 3. Run the hello world sample
echo -e "${BLUE}occlum run /bin/web_server${NC}"
occlum run /bin/web_server
