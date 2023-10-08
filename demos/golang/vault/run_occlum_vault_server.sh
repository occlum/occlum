#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

vault="./source_code/bin/vault"

if [ ! -f $vault ];then
    echo "Error: cannot stat file '$vault'"
    echo "Please see README and build it using Occlum Golang toolchain"
    exit 1
fi

# 1. Init Occlum Workspace
rm -rf occlum_instance
occlum new occlum_instance
cd occlum_instance
new_json="$(jq '.resource_limits.user_space_size = "1MB" |
	.resource_limits.user_space_max_size = "2560MB" |
	.resource_limits.kernel_space_heap_size="1MB" |
	.resource_limits.kernel_space_heap_max_size="320MB" |
	.resource_limits.kernel_space_stack_size="10MB" |
	.resource_limits.max_num_of_threads=64 |
	.process.default_stack_size = "40MB" |
	.process.default_heap_size = "320MB" ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy executable into Occlum Workspace and build
rm -rf image && \
copy_bom -f ../vault.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# 3. Run the Hashicorp Vault server listening on "127.0.0.1:8200" 
echo -e "${BLUE}occlum run /bin/vault server -dev -dev-no-store-token -dev-root-token-id mytoken${NC}"
time occlum run /bin/vault server -dev -dev-no-store-token -dev-root-token-id mytoken &
