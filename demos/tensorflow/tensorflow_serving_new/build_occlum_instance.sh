#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
export TF_DIR="${script_dir}/tf_serving"

function build_tf_instance()
{
    rm -rf occlum_instance
    occlum new occlum_instance
    pushd occlum_instance

    # prepare tf_serving content
    rm -rf image
    copy_bom -f ../tf_serving.yaml --root image --include-dir /opt/occlum/etc/template

    new_json="$(jq '.resource_limits.user_space_size = "1MB" |
                    .resource_limits.user_space_max_size = "7000MB" |
                    .resource_limits.kernel_space_heap_size="1MB" |
                    .resource_limits.kernel_space_heap_max_size="384MB" |
                    .resource_limits.max_num_of_threads = 64 ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    occlum build
    popd
}

build_tf_instance
