#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
INSTALL_DIR=/opt/occlum/toolchains/aecs_client
OCCLUM_INSTALL_DIR=/usr/local/occlum/x86_64-linux-gnu/lib
AECS_DIR=${script_dir}/enclave-configuration-service

# Default TEE TYPE is SGX2, also support HYPERENCLAVE
TEETYPE=${1:-SGX2}

git clone -b occlum-init-ra https://github.com/occlum/enclave-configuration-service.git

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
cp $OCCLUM_INSTALL_DIR/libcurl_static.a ${INSTALL_DIR}/
[ -f $OCCLUM_INSTALL_DIR/libssl.so ] && cp $OCCLUM_INSTALL_DIR/libssl.so* ${INSTALL_DIR}/
[ -f $OCCLUM_INSTALL_DIR/libcrypto.so ] && cp $OCCLUM_INSTALL_DIR/libcrypto.so* ${INSTALL_DIR}/
popd

# Clean up
rm -rf /usr/local/occlum/x86_64-linux-gnu
rm -rf /opt/occlum/toolchains/gcc/x86_64-linux-gnu

popd
