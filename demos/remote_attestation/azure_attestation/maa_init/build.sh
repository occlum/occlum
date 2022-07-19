#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
INSTANCE_DIR="occlum_instance"
IMG_BOM="../bom.yaml"
INIT_BOM="../init_maa.yaml"

function build() {
    pushd init
    cargo clean
    cargo build --release
    popd

    echo "Generate example base64 encoded string as report data"
    openssl genrsa -out key.pem 2048
    report_data=$(base64 -w 0 key.pem)

    rm -rf ${INSTANCE_DIR} && occlum new ${INSTANCE_DIR}
    pushd ${INSTANCE_DIR}

    rm -rf image
    copy_bom -f ${IMG_BOM} --root image --include-dir /opt/occlum/etc/template

    # Update env
    new_json="$(jq '.env.default += ["MAA_PROVIDER_URL=https://shareduks.uks.attest.azure.net"] |
        .env.default += ["MAA_TOKEN_PATH=/root"] |
        .env.default += ["MAA_REPORT_DATA=BASE64_STRING"]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    # Update report data string
    sed -i "s/BASE64_STRING/$report_data/g" Occlum.json

    # prepare init maa content
    rm -rf initfs
    copy_bom -f ${INIT_BOM} --root initfs --include-dir /opt/occlum/etc/template

    occlum build

    popd
}

build


