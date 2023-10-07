#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
python_dir="$script_dir/occlum_instance/image/opt/python-occlum"


function build_instance()
{
    rm -rf occlum_instance && occlum new occlum_instance
    pushd occlum_instance
    rm -rf image
    copy_bom -f ../llm.yaml --root image --include-dir /opt/occlum/etc/template

    new_json="$(jq '.resource_limits.user_space_size = "60GB" |
                    .resource_limits.kernel_space_heap_size = "1GB" |
                    .resource_limits.max_num_of_threads = 500 |
                    .env.default += ["PYTHONHOME=/opt/python-occlum"] |
                    .env.default += ["PATH=/bin"] |
                    .env.default += ["HOME=/root"] |
                    .env.untrusted += ["HF_DATASETS_CACHE", "OMP_NUM_THREADS"]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    # Make model as hostfs mount for test purpose
    # The model should be protected in production by encryption
    mkdir -p image/models
    new_json="$(cat Occlum.json | jq '.mount+=[{"target": "/models", "type": "hostfs","source": "/work/models"}]')" && \
    echo "${new_json}" > Occlum.json

    occlum build
    popd
}

build_instance

