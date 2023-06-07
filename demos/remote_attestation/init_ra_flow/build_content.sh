#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

export DEP_LIBS_DIR="${script_dir}/dep_libs"
export INITRA_DIR="${script_dir}/init_ra"
export FLASK_DIR="${script_dir}/../../python/flask"
export RATLS_DIR="${script_dir}/../../ra_tls"

function build_ratls()
{
    rm -rf ${DEP_LIBS_DIR} && mkdir ${DEP_LIBS_DIR}
    pushd ${RATLS_DIR}
    ./download_and_prepare.sh
    ./build_and_install.sh musl
    ./build_occlum_instance.sh musl

    cp ./grpc-src/examples/cpp/ratls/build/libgrpc_ratls_client.so ${DEP_LIBS_DIR}/
    cp ./grpc-src/examples/cpp/ratls/build/libhw_grpc_proto.so ${DEP_LIBS_DIR}/

    popd
}

function build_flask()
{
    pushd ${FLASK_DIR}
    ${FLASK_DIR}/install_python_with_conda.sh
    popd
}

function build_init_ra()
{
    pushd ${INITRA_DIR}
    occlum-cargo clean
    occlum-cargo build --release
    popd
}

function build_client_instance()
{
    # generate client image key
    occlum gen-image-key image_key

    rm -rf occlum_client && occlum new occlum_client
    pushd occlum_client

    # prepare flask content
    rm -rf image
    copy_bom -f ../flask.yaml --root image --include-dir /opt/occlum/etc/template

    yq '.resource_limits.user_space_size.init = "600MB" |
        .resource_limits.kernel_space_heap_size.init = "512MB" |
        .metadata.debuggable = false |
        .metadata.enable_kss = true |
        .metadata.version_number = 88 |
        .env.default += ["PYTHONHOME=/opt/python-occlum"]' -i Occlum.yaml

    occlum build --image-key ../image_key

    # Get server mrsigner.
    # Here client and server use the same signer-key thus using client mrsigner directly.
    jq ' .verify_mr_enclave = "off" |
        .verify_mr_signer = "on" |
        .verify_isv_prod_id = "off" |
        .verify_isv_svn = "off" |
        .verify_config_svn = "off" |
        .verify_enclave_debuggable = "on" |
        .sgx_mrs[0].mr_signer = ''"'`get_mr client mr_signer`'" |
        .sgx_mrs[0].debuggable = false ' ../ra_config_template.json > dynamic_config.json

    # prepare init-ra content
    rm -rf initfs
    copy_bom -f ../init_ra_client.yaml --root initfs --include-dir /opt/occlum/etc/template

    occlum build -f --image-key ../image_key

    popd
}

function get_mr() {
    sgx_sign dump -enclave ${script_dir}/occlum_$1/build/lib/libocclum-libos.signed.so -dumpfile ../metadata_info_$1.txt
    if [ "$2" == "mr_enclave" ]; then
        sed -n -e '/enclave_hash.m/,/metadata->enclave_css.body.isv_prod_id/p' ../metadata_info_$1.txt |head -3|tail -2|xargs|sed 's/0x//g'|sed 's/ //g'
    elif [ "$2" == "mr_signer" ]; then
        tail -2 ../metadata_info_$1.txt |xargs|sed 's/0x//g'|sed 's/ //g'
    fi
}

function gen_secret_json() {
    # First generate cert/key by openssl
    ./gen-cert.sh

    # Then do base64 encode
    cert=$(base64 -w 0 flask.crt)
    key=$(base64 -w 0 flask.key)
    image_key=$(base64 -w 0 image_key)

    # Then generate secret json
    jq -n --arg cert "$cert" --arg key "$key" --arg image_key "$image_key" \
        '{"flask_cert": $cert, "flask_key": $key, "image_key": $image_key}' >  secret_config.json
}

function build_server_instance()
{
    gen_secret_json
    rm -rf occlum_server && occlum new occlum_server
    pushd occlum_server

    jq '.verify_mr_enclave = "on" |
        .verify_mr_signer = "on" |
        .verify_isv_prod_id = "off" |
        .verify_isv_svn = "on" |
        .verify_config_svn = "on" |
        .verify_enclave_debuggable = "on" |
        .sgx_mrs[0].mr_enclave = ''"'`get_mr client mr_enclave`'" |
        .sgx_mrs[0].mr_signer = ''"'`get_mr client mr_signer`'" |
        .sgx_mrs[0].isv_svn = 88 |
        .sgx_mrs[0].config_svn = 1234 |
        .sgx_mrs[0].debuggable = false ' ../ra_config_template.json > dynamic_config.json

    yq '.resource_limits.user_space_size.init = "500MB" |
        .metadata.debuggable = false ' -i Occlum.yaml

    rm -rf image
    copy_bom -f ../ra_server.yaml --root image --include-dir /opt/occlum/etc/template

    occlum build

    popd
}

build_ratls
build_flask
build_init_ra

build_client_instance
build_server_instance
