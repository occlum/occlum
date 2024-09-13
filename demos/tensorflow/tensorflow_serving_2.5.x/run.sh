#!/bin/bash
set -e

echo "Start Tensorflow-Serving on backgound ..."

pushd occlum_instance
taskset -c 0,1 occlum run /bin/tensorflow_model_server \
        --model_name=resnet --model_base_path=/models/resnet \
        --port=9000 --ssl_config_file="/etc/ssl.cfg"
popd
