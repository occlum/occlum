#!/bin/bash
set -e

INSTALL_DIR=/usr/local/occlum/x86_64-linux-musl
OCCLUM_GCC_INC_DIR=/usr/local/occlum/include
GRPC_VER=1.24.3
GRPC_SRC_DIR=$PWD/grpc-src
CARES_VER=1_15_0
RPOTOBUF_VER=3.10.0

export PATH=$PATH:$INSTALL_DIR/bin
# Tell CMake to search for packages in Occlum toolchain's directory only
export PKG_CONFIG_LIBDIR=$INSTALL_DIR/lib

# Download grpc
wget https://github.com/grpc/grpc/archive/refs/tags/v${GRPC_VER}.tar.gz
rm -rf ${GRPC_SRC_DIR} && mkdir ${GRPC_SRC_DIR}
tar zxvf v${GRPC_VER}.tar.gz -C ${GRPC_SRC_DIR} --strip-components 1

# Download and Install c-ares
wget https://github.com/c-ares/c-ares/archive/refs/tags/cares-${CARES_VER}.tar.gz
tar zxvf cares-${CARES_VER}.tar.gz -C ${GRPC_SRC_DIR}/third_party/cares/cares/ --strip-components 1
cd $GRPC_SRC_DIR/third_party/cares/cares
mkdir -p build
cd build
cmake ../ \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=occlum-gcc \
	-DCMAKE_INSTALL_PREFIX=$INSTALL_DIR

make -j$(nproc)
make install

cd $PWD

# Download and Install protobuf
wget https://github.com/protocolbuffers/protobuf/archive/refs/tags/v${RPOTOBUF_VER}.tar.gz
tar zxvf v${RPOTOBUF_VER}.tar.gz -C $GRPC_SRC_DIR/third_party/protobuf/ --strip-components 1
cd $GRPC_SRC_DIR/third_party/protobuf
cd cmake
mkdir -p build
cd build
cmake ../ \
	-Dprotobuf_BUILD_TESTS=OFF -DBUILD_SHARED_LIBS=TRUE \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=occlum-gcc \
	-DCMAKE_CXX_COMPILER=occlum-g++ -DCMAKE_INSTALL_PREFIX=$INSTALL_DIR \
	-DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE \
	-DCMAKE_INSTALL_OLDINCLUDEDIR=$OCCLUM_GCC_INC_DIR \
	-DZLIB_INCLUDE_DIR=$OCCLUM_GCC_INC_DIR

make -j$(nproc)
make install

cp $INSTALL_DIR/bin/protoc /usr/bin

# Install gRPC
cd $GRPC_SRC_DIR/cmake
mkdir -p build
cd build
cmake ../.. \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=occlum-gcc \
	-DCMAKE_CXX_COMPILER=occlum-g++ -DgRPC_INSTALL=ON -DgRPC_PROTOBUF_PROVIDER=package \
	-DgRPC_ZLIB_PROVIDER=package -DgRPC_CARES_PROVIDER=package \
	-DgRPC_SSL_PROVIDER=package -DCMAKE_PREFIX_PATH=$INSTALL_DIR \
	-DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE -DCMAKE_INSTALL_PREFIX=$INSTALL_DIR

make -j$(nproc)
make install
echo "gRPC build success"
