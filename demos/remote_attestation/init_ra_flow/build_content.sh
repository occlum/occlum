#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

export FLASK_DIR="${script_dir}/../../python/flask"

function build_flask()
{
    pushd ${FLASK_DIR}
    ${FLASK_DIR}/install_python_with_conda.sh
    popd
}

function update_client_init_ra_conf()
{
    # Fill in the keys
    new_json="$(jq '.kms_keys = [ {"key": "flask_cert", "path": "/etc/flask.crt"}] |
        .kms_keys += [ {"key": "flask_key", "path": "/etc/flask.key"}]' init_ra_conf.json)" && \
    echo "${new_json}" > init_ra_conf.json

    # Fill in the KMS server measurements.
    new_json="$(jq ' .ra_config.verify_mr_enclave = "off" |
        .ra_config.verify_mr_signer = "on" |
        .ra_config.verify_isv_prod_id = "off" |
        .ra_config.verify_isv_svn = "off" |
        .ra_config.verify_config_svn = "off" |
        .ra_config.verify_enclave_debuggable = "on" |
        .ra_config.sgx_mrs[0].mr_signer = ''"'`get_mr client mrsigner`'" |
        .ra_config.sgx_mrs[0].debuggable = true ' init_ra_conf.json)" && \
    echo "${new_json}" > init_ra_conf.json
}

function build_client_instance()
{
    # generate client image key
    occlum gen-image-key image_key

    rm -rf occlum_client
    # choose grpc_ratls as init ra kms client
    occlum new occlum_client --init-ra grpc_ratls
    pushd occlum_client

    # prepare flask content
    rm -rf image
    copy_bom -f ../flask.yaml --root image --include-dir /opt/occlum/etc/template

    # Try build first to get mrsigner
    # In our case, client and server use the same sign-key thus also the same mrsigner
    occlum build

    new_json="$(jq '.resource_limits.user_space_size = "600MB" |
        .resource_limits.kernel_space_heap_size = "128MB" |
        .resource_limits.max_num_of_threads = 32 |
        .metadata.debuggable = true |
        .metadata.enable_kss = true |
        .metadata.version_number = 88 |
        .env.default += ["PYTHONHOME=/opt/python-occlum"] ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    # Update init_ra_conf json file accordingly before occlum build
    update_client_init_ra_conf

    occlum build -f --image-key ../image_key

    popd
}

function get_mr() {
    cd ${script_dir}/occlum_$1 && occlum print $2
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
        .sgx_mrs[0].mr_enclave = ''"'`get_mr client mrenclave`'" |
        .sgx_mrs[0].mr_signer = ''"'`get_mr client mrsigner`'" |
        .sgx_mrs[0].isv_svn = 88 |
        .sgx_mrs[0].config_svn = 1234 |
        .sgx_mrs[0].debuggable = true ' ../ra_config_template.json > dynamic_config.json

    new_json="$(jq '.resource_limits.user_space_size = "500MB" |
                    .metadata.debuggable = true ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    rm -rf image
    copy_bom -f ../ra_server.yaml --root image --include-dir /opt/occlum/etc/template

    occlum build

    popd
}

build_flask
build_client_instance
build_server_instance
