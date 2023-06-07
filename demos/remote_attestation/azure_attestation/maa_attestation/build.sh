#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
INSTANCE_DIR="occlum_instance"
bomfile="../bom.yaml"

function build() {
    pushd azure_att
    cargo clean
    cargo build
    popd

    rm -rf ${INSTANCE_DIR} && occlum new ${INSTANCE_DIR}
    pushd ${INSTANCE_DIR}

    rm -rf image
    copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template
    yq '.resource_limits.user_space_size.init = "600MB" |
        .resource_limits.kernel_space_heap_size.init = "512MB" ' -i Occlum.yaml

    occlum build

    popd
}

build


