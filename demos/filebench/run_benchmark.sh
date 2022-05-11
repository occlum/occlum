#!/bin/bash
set -e

# 1. Init Occlum Workspace
rm -rf occlum_instance && occlum new occlum_instance
cd occlum_instance

# 2. Copy files into Occlum Workspace and build
rm -rf image
copy_bom -f ../filebench.yaml --root image --include-dir /opt/occlum/etc/template

# Enlarge "kernel_space_heap_size" when "pre-allocating files failed" occurs
# Enlarge "user_space_size" when "procflow exec proc failed" occurs
TCS_NUM=$(($(nproc) * 2))
new_json="$(jq --argjson THREAD_NUM ${TCS_NUM} '.resource_limits.user_space_size = "5000MB" |
                .resource_limits.kernel_space_heap_size ="512MB" |
                .resource_limits.max_num_of_threads = $THREAD_NUM' Occlum.json)" && \
echo "${new_json}" > Occlum.json

occlum build

# 3. Run benchmark under different workloads
BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Run filebench in Occlum.${NC}"

# More about workload model language at
# https://github.com/filebench/filebench/wiki/Workload-model-language

# Option: videoserver.f fileserver.f varmail.f oltp.f
WORKLOAD_FILE="readfiles.f"
occlum run /bin/filebench -f /workloads/${WORKLOAD_FILE}
