#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
registry=${1:-demo}

pushd ${script_dir}

echo "Build Occlum init-ra Server runtime container image ..."
./container/build_image.sh \
    -i ./occlum_server/occlum_instance.tar.gz \
    -n init_ra_server -r ${registry}

echo "Build Occlum Tensorflow-serving runtime container image ..."
./container/build_image.sh \
    -i ./occlum_tf/occlum_instance.tar.gz \
    -n tf_demo -r ${registry}

popd
