#!/bin/bash
set -e

GRPC_SERVER_DOMAIN=${1:-localhost}
GRPC_SERVER_PORT=${2:-50051}
GRPC_SERVER="${GRPC_SERVER_DOMAIN}:${GRPC_SERVER_PORT}"

echo "Start GRPC server on backgound ..."

pushd occlum_server
occlum run /bin/server ${GRPC_SERVER} &
popd

sleep 3

echo "Start Tensorflow-Serving on backgound ..."

pushd occlum_tf
taskset -c 0,1 occlum run /bin/tensorflow_model_server \
        --model_name=INCEPTION --model_base_path=/model/INCEPTION/INCEPTION \
        --port=9000 --ssl_config_file="/etc/tf_ssl.cfg"
popd
