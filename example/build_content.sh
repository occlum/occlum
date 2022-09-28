#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

export DEP_LIBS_DIR="${script_dir}/dep_libs"
export INITRA_DIR="${script_dir}/init_ra"
export RATLS_DIR="${script_dir}/../demos/ra_tls"
export TF_DIR="${script_dir}/tf_serving"

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

function build_tf_serving()
{
    # Dump tensorflow/serving container rootfs content
    ./dump_rootfs.sh -i tensorflow/serving -d ${TF_DIR} -g 2.5.1
    pushd ${TF_DIR}
    # Download pretrained inception model
    rm -rf INCEPTION*
    curl -O https://s3-us-west-2.amazonaws.com/tf-test-models/INCEPTION.zip
    unzip INCEPTION.zip
    popd
}

function build_init_ra()
{
    pushd ${INITRA_DIR}
    occlum-cargo clean
    occlum-cargo build --release
    popd
}

function build_tf_instance()
{
    # generate tf image key
    occlum gen-image-key image_key

    rm -rf occlum_tf && occlum new occlum_tf
    pushd occlum_tf

    # prepare tf_serving content
    rm -rf image
    copy_bom -f ../tf_serving.yaml --root image --include-dir /opt/occlum/etc/template

    new_json="$(jq '.resource_limits.user_space_size = "7000MB" |
                    .resource_limits.kernel_space_heap_size="384MB" |
                    .process.default_heap_size = "128MB" |
                    .resource_limits.max_num_of_threads = 64 |
                    .metadata.debuggable = false |
                    .env.default += ["GRPC_SERVER=localhost:50051"] |
                    .env.untrusted += ["GRPC_SERVER"]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    occlum build --image-key ../image_key

    # Get server mrsigner.
    # Here client and server use the same signer-key thus using client mrsigner directly.
    jq ' .verify_mr_enclave = "off" |
        .verify_mr_signer = "on" |
        .verify_isv_prod_id = "off" |
        .verify_isv_svn = "off" |
        .verify_config_svn = "off" |
        .verify_enclave_debuggable = "on" |
        .sgx_mrs[0].mr_signer = ''"'`get_mr tf mr_signer`'" |
        .sgx_mrs[0].debuggable = false ' ../ra_config_template.json > dynamic_config.json

    # prepare init-ra content
    rm -rf initfs
    copy_bom -f ../init_ra_client.yaml --root initfs --include-dir /opt/occlum/etc/template

    occlum build -f --image-key ../image_key
    occlum package occlum_instance

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
    ./generate_ssl_config.sh localhost

    # Then do base64 encode
    ssl_config=$(base64 -w 0 ssl_configure/ssl.cfg)
    image_key=$(base64 -w 0 image_key)

    # Then generate secret json
    jq -n --arg ssl_config "$ssl_config" --arg image_key "$image_key" \
        '{"ssl_config": $ssl_config, "image_key": $image_key}' >  secret_config.json
}

function build_server_instance()
{
    gen_secret_json
    rm -rf occlum_server && occlum new occlum_server
    pushd occlum_server

    jq '.verify_mr_enclave = "on" |
        .verify_mr_signer = "on" |
        .verify_isv_prod_id = "off" |
        .verify_isv_svn = "off" |
        .verify_config_svn = "off" |
        .verify_enclave_debuggable = "on" |
        .sgx_mrs[0].mr_enclave = ''"'`get_mr tf mr_enclave`'" |
        .sgx_mrs[0].mr_signer = ''"'`get_mr tf mr_signer`'" |
        .sgx_mrs[0].debuggable = false ' ../ra_config_template.json > dynamic_config.json 

    new_json="$(jq '.resource_limits.user_space_size = "500MB" |
                    .metadata.debuggable = false ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    rm -rf image
    copy_bom -f ../ra_server.yaml --root image --include-dir /opt/occlum/etc/template

    occlum build
    occlum package occlum_instance

    popd
}

build_ratls
build_tf_serving
build_init_ra

build_tf_instance
build_server_instance
