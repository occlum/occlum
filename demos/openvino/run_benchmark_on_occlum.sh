#!/bin/bash
set -e

benchmark=benchmark_app
SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/openvino.yaml

# 1. Init Occlum Workspace
rm -rf occlum_instance
mkdir occlum_instance
cd occlum_instance
occlum init
new_json="$(jq '.resource_limits.max_num_of_threads = 64 |
                .resource_limits.user_space_size = "1MB" |
                .resource_limits.user_space_max_size = "2GB" |
                .env.default = [ "LD_LIBRARY_PATH=/usr/local/openvino/runtime/lib/intel64" ] ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy files into Occlum Workspace and Build
rm -rf image
# make sure libiomp5.so could be found by copy_bom
cp /usr/local/openvino/runtime/lib/intel64/libiomp5.so /usr/local/lib/
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

# 3. Run benchmark
occlum run /bin/$benchmark -m /model/age-gender-recognition-retail-0013.xml
