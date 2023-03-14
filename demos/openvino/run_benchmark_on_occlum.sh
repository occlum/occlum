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

yq '.resource_limits.user_space_size.max = "320MB"' -i Occlum.yaml

# 2. Copy files into Occlum Workspace and Build
rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

# 3. Run benchmark
occlum run /bin/$benchmark -m /model/age-gender-recognition-retail-0013.xml
