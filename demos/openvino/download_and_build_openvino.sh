#!/bin/bash
set -e

export OPENVINO_DIR="openvino_src"
OPENVINO_VERSION="2023.3.0"
PREFIX="/usr/local/openvino"

download_and_build_openvino() {
    rm -rf $OPENVINO_DIR
    git clone https://github.com/openvinotoolkit/openvino.git $OPENVINO_DIR
    pushd $OPENVINO_DIR
    git checkout -b $OPENVINO_VERSION $OPENVINO_VERSION
    git submodule update --init --recursive
    ./scripts/submodule_update_with_gitee.sh

    mkdir build && cd build
    cmake ../ -DENABLE_INTEL_GPU=OFF \
        -DENABLE_INTEL_GNA=OFF \
        -DENABLE_HETERO=OFF \
        -DENABLE_INTEL_NPU=OFF \
        -DTHREADING=OMP \
        -DCMAKE_INSTALL_PREFIX=$PREFIX \
        -DCMAKE_BUILD_TYPE=Release
    make --jobs=$(nproc --all)
    make install
    popd
}

# build benchmark_app
build_sample() {
    pushd $PREFIX/samples/cpp
    ./build_samples.sh -b benchmark_app
    popd
}

download_and_build_openvino
build_sample
