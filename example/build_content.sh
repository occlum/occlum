#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
export TF_DIR="${script_dir}/tf_serving"


function build_tf_serving()
{
    # Dump tensorflow/serving container rootfs content
    ./dump_rootfs.sh -i tensorflow/serving -d ${TF_DIR} -g 2.5.1
    pushd ${TF_DIR}
    # Download pretrained resnet model
    rm -rf resnet*
    wget https://tfhub.dev/tensorflow/resnet_50/classification/1?tf-hub-format=compressed -O resnet.tar.gz
    mkdir -p resnet/123
    tar zxf resnet.tar.gz -C resnet/123
    popd
}

function update_client_init_ra_conf()
{
    # Fill in the keys
    new_json="$(jq '.kms_keys = [ {"key": "ssl_config", "path": "/etc/tf_ssl.cfg"}]' init_ra_conf.json)" && \
    echo "${new_json}" > init_ra_conf.json

    # Fill in the KMS server measurements.
    new_json="$(jq ' .ra_config.verify_mr_enclave = "off" |
        .ra_config.verify_mr_signer = "on" |
        .ra_config.verify_isv_prod_id = "off" |
        .ra_config.verify_isv_svn = "off" |
        .ra_config.verify_config_svn = "off" |
        .ra_config.verify_enclave_debuggable = "on" |
        .ra_config.sgx_mrs[0].mr_signer = ''"'`get_mr tf mrsigner`'" |
        .ra_config.sgx_mrs[0].debuggable = false ' init_ra_conf.json)" && \
    echo "${new_json}" > init_ra_conf.json
}

function build_tf_instance()
{
    # generate tf image key
    occlum gen-image-key image_key

    rm -rf occlum_tf
    # choose grpc_ratls as init ra kms client
    occlum new occlum_tf --init-ra grpc_ratls
    pushd occlum_tf

    # prepare tf_serving content
    rm -rf image
    copy_bom -f ../tf_serving.yaml --root image --include-dir /opt/occlum/etc/template

    # Try build first to get mrsigner
    # In our case, client and server use the same sign-key thus also the same mrsigner
    occlum build

    new_json="$(jq '.resource_limits.user_space_size = "7000MB" |
                    .resource_limits.kernel_space_heap_size="384MB" |
                    .process.default_heap_size = "128MB" |
                    .resource_limits.max_num_of_threads = 64 |
                    .metadata.debuggable = false |
                    .env.default += ["OCCLUM_INIT_RA_KMS_SERVER=localhost:50051"] |
                    .env.untrusted += ["OCCLUM_INIT_RA_KMS_SERVER"]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    # Update init_ra_conf json file accordingly before occlum build
    update_client_init_ra_conf

    occlum build --image-key ../image_key
    occlum package occlum_instance

    popd
}

function get_mr() {
    cd ${script_dir}/occlum_$1 && occlum print $2
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
        .sgx_mrs[0].mr_enclave = ''"'`get_mr tf mrenclave`'" |
        .sgx_mrs[0].mr_signer = ''"'`get_mr tf mrsigner`'" |
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

build_tf_serving

build_tf_instance
build_server_instance
