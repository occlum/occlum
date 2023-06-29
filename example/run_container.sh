#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

grpc_domain=localhost
grpc_port=50051
pccs_url="https://localhost:8081/sgx/certification/v3/"
registry="demo"
tag="latest"

function usage {
    cat << EOM
Run container images init_ra_server and tf_demo on background.
usage: $(basename "$0") [OPTION]...
    -s <GRPC Server Domain> default localhost.
    -p <GRPC Server port> default 50051.
    -u <PCCS URL> default https://localhost:8081/sgx/certification/v3/.
    -r <registry prefix> the registry for this demo container images.
    -g <image tag> the container images tag, default it is "latest".
    -h <usage> usage help
EOM
    exit 0
}

function process_args {
    while getopts ":s:p:u:r:g:h" option; do
        case "${option}" in
            s) grpc_domain=${OPTARG};;
            p) grpc_port=${OPTARG};;
            u) pccs_url=${OPTARG};;
            r) registry=${OPTARG};;
            g) tag=${OPTARG};;
            h) usage;;
        esac
    done
}

process_args "$@"

echo "Start GRPC server on backgound ..."

docker run --network host \
        --device /dev/sgx/enclave --device /dev/sgx/provision \
        --env PCCS_URL=${pccs_url} \
        ${registry}/init_ra_server:${tag} \
        occlum run /bin/server ${grpc_domain}:${grpc_port} &

sleep 3

echo "Start Tensorflow-Serving on backgound ..."
GRPC_SERVER="${grpc_domain}:${grpc_port}"

docker run --network host \
        --device /dev/sgx/enclave --device /dev/sgx/provision \
        --env PCCS_URL=${pccs_url} \
        --env OCCLUM_INIT_RA_KMS_SERVER="${GRPC_SERVER}" \
        ${registry}/tf_demo:${tag} \
        taskset -c 0,1 occlum run /bin/tensorflow_model_server \
        --model_name=resnet --model_base_path=/models/resnet \
        --port=9000 --ssl_config_file="/etc/tf_ssl.cfg" &
