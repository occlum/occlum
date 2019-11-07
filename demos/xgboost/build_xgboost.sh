#!/bin/bash
set -e

# Install the dependencies
apt-get update
apt-get install -y python3.5
apt-get install -y python3-pip
apt-get install -y python3-setuptools
apt-get install -y python-pip
apt-get install -y python-setuptools
pip3 install kubernetes
pip install kubernetes

# Download and build XGBoost
rm -rf xgboost_src && mkdir xgboost_src
pushd xgboost_src
git clone https://github.com/dmlc/xgboost .
git checkout 6d5b34d82486cd1d0480c548f5d1953834659bd6
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
make -j
popd

# Prepare data
pushd xgboost_src/demo/binary_classification
python mapfeat.py
python mknfold.py agaricus.txt 1
popd

echo "Build XGBoost Success!"
