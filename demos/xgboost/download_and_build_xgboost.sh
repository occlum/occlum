#!/bin/bash
set -e

# Tell CMake to search for packages in Occlum toolchain's directory only
export PKG_CONFIG_LIBDIR=/usr/local/occlum/x86_64-linux-musl/lib

# Install the dependencies
OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y python3-pip python3-setuptools
else
  yum install -y python3-pip python3-setuptools
fi
pip3 install kubernetes

#install the cmake
./install_cmake.sh

# Download and build XGBoost
rm -rf xgboost_src && mkdir xgboost_src
pushd xgboost_src
git clone https://github.com/dmlc/xgboost .
git checkout 9e955fb9b06cac32a06c92c4715f749d9d87e932
git submodule init
git submodule update
git apply ../patch/xgboost-01.diff
pushd rabit
git apply ../../patch/rabit-01.diff
popd
pushd dmlc-core
git apply ../../patch/dmlc-core-01.diff
popd
mkdir build
cd build
cmake ../ \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=occlum-gcc -DCMAKE_CXX_COMPILER=occlum-g++
make -j4
popd

# Prepare data
pushd xgboost_src/demo/binary_classification
python mapfeat.py
python mknfold.py agaricus.txt 1
popd

echo "Build XGBoost Success!"
