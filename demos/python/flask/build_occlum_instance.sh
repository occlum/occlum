#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
python_dir="$script_dir/occlum_instance/image/opt/python-occlum"

rm -rf occlum_instance && occlum new occlum_instance
cd occlum_instance && rm -rf image
copy_bom -f ../flask.yaml --root image --include-dir /opt/occlum/etc/template

if [ ! -d $python_dir ];then
    echo "Error: cannot stat '$python_dir' directory"
    exit 1
fi

new_json="$(jq '.resource_limits.user_space_size = "1MB" |
        .resource_limits.user_space_max_size = "640MB" |
        .resource_limits.kernel_space_heap_size = "1MB" |
        .resource_limits.kernel_space_heap_max_size = "256MB" |
        .env.default += ["PYTHONHOME=/opt/python-occlum"]' Occlum.json)" && \
echo "${new_json}" > Occlum.json
occlum build

