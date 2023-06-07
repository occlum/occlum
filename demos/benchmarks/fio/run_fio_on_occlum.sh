#!/bin/bash
set -e

GREEN='\033[1;32m'
NC='\033[0m'

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/fio.yaml

FIO=fio
FIO_CONFIG=$1
FIO_PATH=$2

if [ ! -e ${SCRIPT_DIR}/fio_src/${FIO} ];then
    echo "Error: cannot stat '${FIO} in fio_src'"
    echo "Please see README and build the ${FIO}"
    exit 1
fi

if [ ! -e ${SCRIPT_DIR}/configs/${FIO_CONFIG} ]  || [ -z ${FIO_CONFIG} ];then
    echo "Error: cannot stat '${FIO_CONFIG}' in configs"
    echo "Please copy it into configs first"
    exit 1
fi

# 1. Init Occlum instance
rm -rf occlum_instance && occlum new occlum_instance
cd occlum_instance
yq '.resource_limits.user_space_size.init = "320MB" |
    .resource_limits.kernel_space_heap_size.init = "800MB" |
    .resource_limits.kernel_space_heap_size.max = "800MB" ' -i Occlum.yaml

# 2. Copy files into Occlum instance and build
rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

# 3. Run the program
echo -e "${GREEN}occlum run /bin/${FIO} /configs/${FIO_CONFIG} --filename=${FIO_PATH}${NC}"
occlum run /bin/${FIO} "/configs/${FIO_CONFIG}" "--filename=${FIO_PATH}"
