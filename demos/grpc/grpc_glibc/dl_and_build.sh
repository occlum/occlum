#!/bin/bash
set -e

GRPC_VERSION=v1.48.4
GRPC_PATH=grpc_src

# Download and update cmake
function dl_and_build_cmake() {
    # Ubuntu 20.04/22.04 has newer enough cmake version
    if [ -f "/etc/os-release" ]; then
        local os_name=$(cat /etc/os-release)
        if [[ $os_name =~ "Ubuntu" ]]; then
            return
        fi
    fi

    rm -rf cmake-3.20.2*
    wget https://github.com/Kitware/CMake/releases/download/v3.20.2/cmake-3.20.2.tar.gz
    tar -zxvf cmake-3.20.2.tar.gz
    pushd cmake-3.20.2
    ./bootstrap
    make install
    popd
}

# GRPC env
function dl_grpc() {
    # GRPC source code
    rm -rf ${GRPC_PATH}
    git clone https://github.com/grpc/grpc -b ${GRPC_VERSION} ${GRPC_PATH}
    pushd ${GRPC_PATH}
    git submodule update --init

    # build and install abseil library
    # https://abseil.io/docs/cpp/quickstart-cmake.html
    mkdir -p third_party/abseil-cpp/build
    pushd third_party/abseil-cpp/build
    cmake -DCMAKE_CXX_STANDARD=11 -DCMAKE_POSITION_INDEPENDENT_CODE=TRUE \
            -DCMAKE_BUILD_TYPE=${BUILD_TYPE} ..
    make -j$(nproc)
    make install
    popd
    
    mkdir -p cmake/build
    pushd cmake/build
    cmake ../.. \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS="-fPIC -pie" -DCMAKE_C_FLAGS="-fPIC -pie" \
        -DgRPC_INSTALL=ON -DgRPC_PROTOBUF_PROVIDER=package \
        -DgRPC_ZLIB_PROVIDER=package -DgRPC_CARES_PROVIDER=package \
        -DgRPC_SSL_PROVIDER=package -DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE

    make -j$(nproc)
    make install
    popd

    # Build helloworld example using cmake
    mkdir -p examples/cpp/helloworld/cmake/build
    pushd examples/cpp/helloworld/cmake/build
    cmake ../..
    make -j$(nproc)
    popd

    popd
}

dl_and_build_cmake
dl_grpc
