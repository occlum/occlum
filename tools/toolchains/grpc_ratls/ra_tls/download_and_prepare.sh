#!/bin/bash
set -e

source ./env.sh

# Download and update cmake
function dl_and_build_cmake() {
    # Ubuntu 20.04 has newer enough cmake version
    if [ -f "/etc/os-release" ]; then
        local os_name=$(cat /etc/os-release)
        if [[ $os_name =~ "Ubuntu" && $os_name =~ "20.04" ]]; then
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
    pushd ${GRPC_PATH} \
        && git checkout ${GRPC_VERSION} \
        && git submodule update --init
    popd
}

# Download cJSON
function dl_cjson() {
    rm -rf cJSON*
    wget https://github.com/DaveGamble/cJSON/archive/refs/tags/v${CJSON_VER}.tar.gz
    tar zxvf v${CJSON_VER}.tar.gz
}


dl_and_build_cmake
dl_grpc
dl_cjson
