#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
INSTANCE_DIR="occlum_instance"

pushd dcap_lib
cargo build --all-targets
popd

make -C c_app

rm -rf ${INSTANCE_DIR} && occlum new ${INSTANCE_DIR}
cd ${INSTANCE_DIR}

rm -rf image
copy_bom -f ../dcap.yaml --root image --include-dir /opt/occlum/etc/template

occlum build

echo -e "${BLUE}occlum run rust test /bin/dcap_test${NC}"
occlum run /bin/dcap_test

echo -e "************"

echo -e "${BLUE}occlum run C test /bin/dcap_c_test${NC}"
occlum run /bin/dcap_c_test
