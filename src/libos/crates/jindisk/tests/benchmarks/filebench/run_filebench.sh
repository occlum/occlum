#!/bin/bash
set -e

GREEN='\033[1;32m'
NC='\033[0m'

DEMO_DIR=../../../../../../../demos
BENCH_DIR=${DEMO_DIR}/benchmarks/filebench

pushd ${BENCH_DIR}

if [ ! -d filebench ];then
    echo -e "${GREEN}Preinstall dependencies${NC}"
    ./preinstall_deps.sh
    echo -e "${GREEN}Download and build Filebench first${NC}"
    ./dl_and_build_filebench.sh
fi

WORKLOAD=$1

echo -e "${GREEN}Running Filebench (workload=${WORKLOAD})${NC}"
./run_workload.sh ${WORKLOAD}

popd
