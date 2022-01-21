#!/bin/bash
set -e

BUILD_TYPE=Release

if [[ $1 == "musl" ]]; then
    echo "*** Build and run musl-libc demo ***"
    CC=occlum-gcc
    CXX=occlum-g++
    CARGO=occlum-cargo
    DCAP_LIB_PATH="target/x86_64-unknown-linux-musl/release"
    INSTALL_PREFIX="/usr/local/occlum/x86_64-linux-musl"
else
    echo "*** Build and run glibc demo ***"
    CC=gcc
    CXX=g++
    CARGO=cargo
    DCAP_LIB_PATH="target/release"
    INSTALL_PREFIX="/usr/local"
fi

# Build occlum dcap lib first
pushd occlum
cd demos/remote_attestation/dcap/dcap_lib
$CARGO build --all-targets --release
cp ${DCAP_LIB_PATH}/libdcap_quote.a ${INSTALL_PREFIX}/lib
cp ../c_app/dcap_quote.h ${INSTALL_PREFIX}/include/
popd

# Copy ratls added/updated files to grpc source
GRPC_PATH=grpc-src
cp -rf grpc/v1.38.1/* ${GRPC_PATH}/

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
mkdir -p build
cd build
cmake -D CMAKE_PREFIX_PATH=${INSTALL_PREFIX} -D CMAKE_BUILD_TYPE=${BUILD_TYPE} \
	-DCMAKE_CXX_COMPILER=${CXX} -DCMAKE_C_COMPILER=${CC} ..
make -j `nproc`
popd
