#!/bin/bash
set -e

INSTALL_DIR=/usr/local/occlum/x86_64-linux-musl
export PATH=$PATH:$INSTALL_DIR/bin
GRPC_SRC_DIR=$PWD/grpc-src
WORK_DIR=$PWD

rm -rf build && mkdir build
pushd build

cp -R ${GRPC_SRC_DIR}/examples/cpp/helloworld/. .
patch Makefile -i ${WORK_DIR}/Makefile.patch

cp ${GRPC_SRC_DIR}/examples/protos/helloworld.proto .
cp ${WORK_DIR}/*.cc .

make -j$(nproc)

popd

# Generate demo ca/csr/crt
./gen-cert.sh

# Build server occlum instance
rm -rf occlum_server
occlum new occlum_server
cd occlum_server

rm -rf image && \
copy_bom -f ../grpc_secure_server.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

if [ $? -ne 0 ]
then
  echo "occlum build failed"
  exit 1
fi

# Build client occlum instance
cd $WORK_DIR
rm -rf occlum_client
occlum new occlum_client
cd occlum_client

rm -rf image && \
copy_bom -f ../grpc_secure_client.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

if [ $? -ne 0 ]
then
  echo "occlum build failed"
  exit 1
fi

