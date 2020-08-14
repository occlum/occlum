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
rm -rf occlum_pong_context && mkdir occlum_pong_context
cd occlum_pong_context
occlum init
sed -i 's/256MB/2560MB/g' ./Occlum.json
sed -i 's/32MB/320MB/g' ./Occlum.json
sed -i 's/1MB/10MB/g' ./Occlum.json
sed -i 's/4MB/40MB/g' ./Occlum.json
sed -i 's/32MB/320MB/g' ./Occlum.json
sed -i 's/80MB/960MB/g' ./Occlum.json

# 2. Copy program into Occlum Workspace and build
cp ../occlum_pong image/bin
mkdir image/etc/
cp /etc/hosts image/etc/
occlum build

# 3. Run the hello world sample
echo -e "${BLUE}occlum run /bin/occlum_pong${NC}"
time occlum run /bin/occlum_pong
