#!/bin/bash
set -e

# Download and update cmake
rm -rf cmake-3.20.2*
wget https://github.com/Kitware/CMake/releases/download/v3.20.2/cmake-3.20.2.tar.gz
tar -zxvf cmake-3.20.2.tar.gz
pushd cmake-3.20.2
./bootstrap
make install
popd

# GRPC env
GRPC_VERSION=v1.38.x
GRPC_PATH=grpc-src

# GRPC source code
rm -rf ${GRPC_PATH}
git clone https://github.com/grpc/grpc -b ${GRPC_VERSION} ${GRPC_PATH}
pushd ${GRPC_PATH} \
    && git checkout v1.38.1 \
    && git submodule update --init
popd


# Download cJSON
CJSON_VER=1.7.15
rm -rf cJSON*
wget https://github.com/DaveGamble/cJSON/archive/refs/tags/v${CJSON_VER}.tar.gz
tar zxvf v${CJSON_VER}.tar.gz

