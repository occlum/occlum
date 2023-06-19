#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
INSTALL_DIR=/opt/occlum/toolchains/aecs_client
AECS_DIR=${script_dir}/enclave-configuration-service

# Default TEE TYPE is SGX2, also support HYPERENCLAVE
TEETYPE=${1:-SGX2}

git clone https://github.com/SOFAEnclave/enclave-configuration-service.git

pushd ${AECS_DIR}
git submodule update --init --recursive

echo "Start building AECS client libraries ..."
pushd client/cpp_occlum
./occlum_build_prepare.sh
./occlum_build_aecs_client.sh --teetype ${TEETYPE} --envtype OCCLUM

echo "Move AECS client libraries to toolchain path"
mkdir -p ${INSTALL_DIR}
cp ./build/out/libaecs_client.so ${INSTALL_DIR}/
cp ./build/out/libual.so ${INSTALL_DIR}/
cp /usr/local/occlum/x86_64-linux-gnu/lib/libcurl_static.a ${INSTALL_DIR}/
popd

# Clean up
rm -rf /usr/local/occlum/x86_64-linux-gnu
rm -rf /opt/occlum/toolchains/gcc/x86_64-linux-gnu

popd
