#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Start building rocksdb from src.${NC}"
TAG="v7.0.4"

rm -rf rocksdb
git clone -b $TAG https://github.com/facebook/rocksdb.git
pushd rocksdb

# Disable fallocate(different st_blksize will cause SIGFPE) and file_sync_range(not implemented) syscalls
sed -i '2467 i CXXFLAGS += -UROCKSDB_FALLOCATE_PRESENT -UROCKSDB_RANGESYNC_PRESENT' Makefile

# Build librocksdb.a and db_bench tool
CFLAGS="-O2 -fPIC" CXXFLAGS="-O2 -fPIC" LDFLAGS="-pie" \
make -j$(nproc) static_lib db_bench DEBUG_LEVEL=0
echo -e "${BLUE}Finish building rocksdb from src.${NC}"

echo -e "${BLUE}Start building simple_rocksdb_example.${NC}"
pushd examples
EXAMPLE=compaction_filter_example
g++ -O2 -std=c++17 -fPIC -pie -fno-rtti $EXAMPLE.cc -osimple_rocksdb_example ../librocksdb.a -I../include \
-lpthread -lrt -ldl -lgflags -lsnappy -lz -lbz2 -llz4 -lzstd
echo -e "${BLUE}Finish building simple_rocksdb_example.${NC}"

./simple_rocksdb_example
popd

popd
