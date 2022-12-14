#!/bin/bash
set -e

# 1. Init Occlum Workspace
rm -rf occlum_instance && occlum new occlum_instance
cd occlum_instance

# 2. Copy files into Occlum Workspace and build
rm -rf image
copy_bom -f ../rocksdb.yaml --root image --include-dir /opt/occlum/etc/template

yq '.resource_limits.user_space_size.init = "1024MB" |
    .resource_limits.kernel_space_heap_size.init ="512MB" ' -i Occlum.yaml

occlum build

# 3. Run example and benchmark with config
BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Run simple_rocksdb_example in Occlum.${NC}"
occlum run /bin/simple_rocksdb_example

echo -e "${BLUE}Run benchmark on Occlum.${NC}"

# More benchmark config at https://github.com/facebook/rocksdb/wiki/Benchmarking-tools
BENCHMARK_CONFIG="fillseq,fillrandom,readseq,readrandom,deleteseq"
occlum run /bin/db_bench --benchmarks=${BENCHMARK_CONFIG}

echo -e "${BLUE}Run benchmark on host.${NC}"
cd ../rocksdb && ./db_bench --benchmarks=$BENCHMARK_CONFIG
