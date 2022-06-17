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
    new_json="$(jq '.resource_limits.user_space_size = "600MB" |
        .resource_limits.kernel_space_heap_size = "128MB"' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    occlum build

    popd
}

build


