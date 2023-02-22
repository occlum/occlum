#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
INSTALL_DIR=/opt/occlum/toolchains/grpc_ratls
RATLS_DIR=${script_dir}/ra_tls

pushd ${RATLS_DIR}
echo "Download and prepare grpc ra_tls"
./download_and_prepare.sh

echo "Build and install musl-libc grpc ra_tls"
./build_and_install.sh musl
mkdir -p ${INSTALL_DIR}/musl
cp ./grpc-src/examples/cpp/ratls/build/libgrpc_ratls_client.so ${INSTALL_DIR}/musl/
cp ./grpc-src/examples/cpp/ratls/build/libgrpc_ratls_server.so ${INSTALL_DIR}/musl/
cp ./grpc-src/examples/cpp/ratls/build/libhw_grpc_proto.so ${INSTALL_DIR}/musl/
cp ./grpc-src/examples/cpp/ratls/build/server ${INSTALL_DIR}/musl/

echo "Build and install glibc grpc ra_tls"
./build_and_install.sh
mkdir -p ${INSTALL_DIR}/glibc
cp ./grpc-src/examples/cpp/ratls/build/libgrpc_ratls_client.so ${INSTALL_DIR}/glibc/
cp ./grpc-src/examples/cpp/ratls/build/libgrpc_ratls_server.so ${INSTALL_DIR}/glibc/
cp ./grpc-src/examples/cpp/ratls/build/libhw_grpc_proto.so ${INSTALL_DIR}/glibc/
cp ./grpc-src/examples/cpp/ratls/build/server ${INSTALL_DIR}/glibc/

# Do clean
rm -rf grpc-src
rm -f *.tar.gz
rm -rf cJSON*
popd


