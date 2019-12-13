#!/bin/sh
install_dir=/usr/local/occlum/x86_64-linux-musl/
export PATH=$PATH:$install_dir/bin

git clone https://github.com/grpc/grpc.git 
cd grpc
git checkout tags/v1.24.3
if [ $? -ne 0 ]
then
  echo "git clone failed"
  exit 1
fi

# Install c-ares
cd third_party/cares/cares
git submodule update --init .
git checkout tags/cares-1_15_0
mkdir -p build
cd build
cmake ../ \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=occlum-gcc \
	-DCMAKE_INSTALL_PREFIX=$install_dir
if [ $? -ne 0 ]
then
  echo "cares cmake failed"
  exit 1
fi
make -j8
if [ $? -ne 0 ]
then
  echo "cares make failed"
  exit 1
fi
make install
cd ../../../..

# Install zlib
cd third_party/zlib
git submodule update --init .
git checkout tags/v1.2.11
mkdir -p build
cd build
cmake ../ \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=occlum-gcc \
	-DCMAKE_CXX_COMPILER=occlum-g++ -DCMAKE_INSTALL_PREFIX=$install_dir \
	-DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE
if [ $? -ne 0 ]
then
  echo "zlib cmake failed"
  exit 1
fi
make -j8
if [ $? -ne 0 ]
then
  echo "zlib make failed"
  exit 1
fi
make install
cd ../../..

# Install protobuf
cd third_party/protobuf
git submodule update --init .
git checkout tags/v3.10.0
cd cmake
mkdir -p build
cd build
cmake ../ \
	-Dprotobuf_BUILD_TESTS=OFF -DBUILD_SHARED_LIBS=TRUE \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=occlum-gcc \
	-DCMAKE_CXX_COMPILER=occlum-g++ -DCMAKE_INSTALL_PREFIX=$install_dir \
	-DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE

if [ $? -ne 0 ]
then
  echo "protobuf cmake failed"
  exit 1
fi

make -j8
if [ $? -ne 0 ]
then
  echo "protobuf make failed"
  exit 1
fi
make install
cd ../../../..

cp $install_dir/bin/protoc /usr/bin

# Install gRPC
cd cmake
mkdir -p build
cd build
cmake ../.. \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=occlum-gcc \
	-DCMAKE_CXX_COMPILER=occlum-g++ -DgRPC_INSTALL=ON -DgRPC_PROTOBUF_PROVIDER=package \
	-DgRPC_ZLIB_PROVIDER=package -DgRPC_CARES_PROVIDER=package \
	-DgRPC_SSL_PROVIDER=package -DCMAKE_PREFIX_PATH=$install_dir \
	-DCMAKE_NO_SYSTEM_FROM_IMPORTED=TRUE -DCMAKE_INSTALL_PREFIX=$install_dir

if [ $? -ne 0 ]
then
  echo "grpc cmake failed"
  exit 1
fi

make -j8
if [ $? -ne 0 ]
then
  echo "grpc make failed"
  exit 1
fi
make install
echo "gRPC build success"
