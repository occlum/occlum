#!/bin/bash
set -e

GRPC_SRC_DIR=$PWD/grpc_src

mkdir -p $GRPC_SRC_DIR && cd $GRPC_SRC_DIR
git clone https://github.com/grpc/grpc.git .
git checkout tags/v1.24.3

# Install c-ares
cd $GRPC_SRC_DIR/third_party/cares/cares
git submodule update --init .
git checkout tags/cares-1_15_0
mkdir -p build
cd build
cmake ../ \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS="-fPIC -pie" -DCMAKE_C_FLAGS="-fPIC -pie"
make -j$(nproc)
make install

# Install protobuf
cd $GRPC_SRC_DIR/third_party/protobuf
git submodule update --init .
git checkout tags/v3.10.0
cd cmake
mkdir -p build
cd build
cmake ../ \
	-Dprotobuf_BUILD_TESTS=OFF -DBUILD_SHARED_LIBS=TRUE \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS="-fPIC -pie" -DCMAKE_C_FLAGS="-fPIC -pie" \
	-DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE
make -j$(nproc)
make install

# Install gRPC
cd $GRPC_SRC_DIR/cmake
mkdir -p build
cd build
cmake ../.. \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS="-fPIC -pie" -DCMAKE_C_FLAGS="-fPIC -pie" \
	-DgRPC_INSTALL=ON -DgRPC_PROTOBUF_PROVIDER=package \
	-DgRPC_ZLIB_PROVIDER=package -DgRPC_CARES_PROVIDER=package \
	-DgRPC_SSL_PROVIDER=package -DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE
make -j$(nproc)
make install

echo "gRPC build success"
