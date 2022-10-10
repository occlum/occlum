#!/bin/bash
set -e

source ./env.sh

BUILD_TYPE=Release

if [[ $1 == "musl" ]]; then
    echo "*** Build and run musl-libc demo ***"
    CC=occlum-gcc
    CXX=occlum-g++
    DCAP_LIB_PATH="/opt/occlum/toolchains/dcap_lib/musl"
    INSTALL_PREFIX="/usr/local/occlum/x86_64-linux-musl"
else
    echo "*** Build and run glibc demo ***"
    CC=gcc
    CXX=g++
    DCAP_LIB_PATH="/opt/occlum/toolchains/dcap_lib/glibc"
    INSTALL_PREFIX="/usr/local"
fi

# Build and install cJSON
function build_cjson() {
    pushd cJSON-${CJSON_VER}
    rm -rf build && mkdir build && cd build
    cmake -DENABLE_CJSON_UTILS=On -DENABLE_CJSON_TEST=Off -DCMAKE_INSTALL_PREFIX=${INSTALL_PREFIX} \
        -DCMAKE_C_COMPILER=${CC} ..
    make install
    popd
}

function build_grpc_ratls() {
    # Copy occlum dcap lib first
    cp ${DCAP_LIB_PATH}/libocclum_dcap.so* ${INSTALL_PREFIX}/lib
    cp /opt/occlum/toolchains/dcap_lib/inc/occlum_dcap.h ${INSTALL_PREFIX}/include/

    # Copy ratls added/updated files to grpc source
    cp -rf grpc/${GRPC_VERSION}/* ${GRPC_PATH}/

    ABSEIL_PATH=${GRPC_PATH}/third_party/abseil-cpp

    # build and install abseil library
    # https://abseil.io/docs/cpp/quickstart-cmake.html
    pushd ${ABSEIL_PATH}
    rm -rf build && mkdir build && cd build
    cmake -DCMAKE_CXX_STANDARD=11 -DCMAKE_POSITION_INDEPENDENT_CODE=TRUE \
            -DCMAKE_BUILD_TYPE=${BUILD_TYPE} -DCMAKE_INSTALL_PREFIX=${INSTALL_PREFIX} \
            -DCMAKE_CXX_COMPILER=${CXX} -DCMAKE_C_COMPILER=${CC} ..
    make -j `nproc`
    make install
    popd

    # Build grpc + ratls
    pushd ${GRPC_PATH}
    rm -rf build && mkdir build && cd build
    cmake -DgRPC_INSTALL=ON -DgRPC_ABSL_PROVIDER=package -DgRPC_BUILD_TESTS=OFF \
        -DgRPC_BUILD_CSHARP_EXT=OFF -DgRPC_BUILD_GRPC_CSHARP_PLUGIN=OFF \
        -DgRPC_BUILD_GRPC_PHP_PLUGIN=OFF -DgRPC_BUILD_GRPC_RUBY_PLUGIN=OFF \
        -DDEFINE_SGX_RA_TLS_OCCLUM_BACKEND=ON \
        -DCMAKE_CXX_COMPILER=${CXX} -DCMAKE_C_COMPILER=${CC} \
        -DCMAKE_BUILD_TYPE=${BUILD_TYPE} -DCMAKE_INSTALL_PREFIX=${INSTALL_PREFIX} ..
    make -j `nproc`
    make install
    popd

    # Build grpc ratls client and server demo
    pushd ${GRPC_PATH}/examples/cpp/ratls
    rm -rf build && mkdir -p build
    cd build
    cmake -D CMAKE_PREFIX_PATH=${INSTALL_PREFIX} -D CMAKE_BUILD_TYPE=${BUILD_TYPE} \
        -DCMAKE_CXX_COMPILER=${CXX} -DCMAKE_C_COMPILER=${CC} ..
    make -j `nproc`
    popd
}

build_cjson
build_grpc_ratls
