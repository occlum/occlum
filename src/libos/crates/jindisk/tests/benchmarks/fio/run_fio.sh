#!/bin/bash
set -e

GREEN='\033[1;32m'
NC='\033[0m'

DEMO_DIR=../../../../../../../demos
FIO_DIR=${DEMO_DIR}/benchmarks/fio

pushd ${FIO_DIR}

if [ ! -d fio_src ];then
    echo -e "${GREEN}Download and build FIO first${NC}"
    ./download_and_build_fio.sh
fi

FIO_CONFIG=$1
TEST_PATH=$2

echo -e "${GREEN}Running FIO (config_file=${FIO_CONFIG} test_path=${TEST_PATH})${NC}"
./run_fio_on_occlum.sh ${FIO_CONFIG} ${TEST_PATH}

popd
