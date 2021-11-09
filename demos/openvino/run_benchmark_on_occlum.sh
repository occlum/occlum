#!/bin/bash
benchmark=benchmark_app
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/openvino.yaml

# 1. Init Occlum Workspace
rm -rf occlum_instance
mkdir occlum_instance
cd occlum_instance
occlum init
cpu_cc=`cat /proc/cpuinfo | grep processor | wc -l`
#new_json="$(jq '.resource_limits.user_space_size = "4GB" |
#                .resource_limits.kernel_space_heap_size = "128MB" |
#                .resource_limits.kernel_space_stack_size = "16MB" |
#                .resource_limits.max_num_of_threads = 128 |
#                .process.default_mmap_size = "1024MB" |
#                .process.default_stack_size = "8MB" |
#                .process.default_heap_size = "32MB" |
#                .metadata.debuggable = false ' Occlum.json)" && \
new_json="$(jq '.resource_limits.user_space_size = "320MB" |
                .process.default_mmap_size = "256MB"' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy files into Occlum Workspace and Build
rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

# 3. Run benchmark
occlum run /bin/$benchmark -m /model/age-gender-recognition-retail-0013.xml
