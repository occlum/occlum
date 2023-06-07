#!/bin/bash
set -e

# 1. Init Occlum Workspace
rm -rf occlum_instance && occlum new occlum_instance
cd occlum_instance

# 2. Copy files into Occlum Workspace and build
rm -rf image
copy_bom -f ../rocksdb.yaml --root image --include-dir /opt/occlum/etc/template

yq '.resource_limits.user_space_size.init = "1024MB" |
    .resource_limits.kernel_space_heap_size.init ="800MB" |
    .resource_limits.kernel_space_heap_size.max ="800MB" ' -i Occlum.yaml

occlum build

# 3. Run example and benchmark with config
BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Run benchmark on Occlum.${NC}"

# More benchmark config at https://github.com/facebook/rocksdb/wiki/Benchmarking-tools
BENCHMARK_CONFIG="fillseq,fillrandom,readseq,readrandom,deleteseq"
DB_DIR=$1

occlum run /bin/db_bench --benchmarks=${BENCHMARK_CONFIG} --db=${DB_DIR}
