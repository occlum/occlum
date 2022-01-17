#!/bin/bash
INSTALL_PREFIX=/usr/local
apt-get update \
    && apt-get install -y --no-install-recommends apt-utils \
    && apt-get install -y \
        ca-certificates \
        build-essential \
        autoconf \
        libtool \
        python3-pip \
        python3-dev \
        git \
        wget \
        unzip

mkdir -p ${INSTALL_PREFIX} \
    && wget -q -O cmake-linux.sh https://github.com/Kitware/CMake/releases/download/v3.19.6/cmake-3.19.6-Linux-x86_64.sh \
    && sh cmake-linux.sh -- --skip-license --prefix=${INSTALL_PREFIX} \
    && rm cmake-linux.sh

# Install cJSON
CJSON_PATH=/cJSON
git clone https://github.com/DaveGamble/cJSON.git ${CJSON_PATH}
pushd ${CJSON_PATH} \
    && make static \
    && cp -r *.a ${INSTALL_PREFIX}/lib \
    && mkdir -p ${INSTALL_PREFIX}/include/cjson \
    && cp -r *.h ${INSTALL_PREFIX}/include/cjson
popd

# GRPC env
GRPC_VERSION=v1.38.x
export GRPC_PATH=/grpc

# GRPC source code
git clone https://github.com/grpc/grpc -b ${GRPC_VERSION} ${GRPC_PATH}
pushd ${GRPC_PATH} \
    && pip3 install --upgrade pip setuptools==44.1.1 \
    && pip3 install -r requirements.txt \
    && git checkout v1.38.1 \
    && git submodule update --init
popd

cp -rf grpc/common/* ${GRPC_PATH}/
cp -rf grpc/v1.38.1/* ${GRPC_PATH}/

git clone  https://github.com/occlum/occlum
pushd occlum
make submodule
cd demos/remote_attestation/dcap/dcap_lib
cargo build --all-targets
cp target/debug/libdcap_quote.a /usr/local/lib/
cp ../c_app/dcap_quote.h /usr/local/include/
popd

pushd ${GRPC_PATH}/examples/cpp/ratls
./build.sh
popd

