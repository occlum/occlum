#!/bin/bash
set -e

GREEN='\033[1;32m'
NC='\033[0m'

DEMO_DIR=../../../../../../../demos
ROCKS_DIR=${DEMO_DIR}/rocksdb

pushd ${ROCKS_DIR}

if [ ! -d rocksdb ];then
    echo -e "${GREEN}Preinstall dependencies${NC}"
    ./preinstall_deps.sh
    echo -e "${GREEN}Download and build RocksDB first${NC}"
    ./dl_and_build_rocksdb.sh
fi

echo -e "${GREEN}Running RocksDB workloads${NC}"
./run_benchmark.sh /sfs

popd
