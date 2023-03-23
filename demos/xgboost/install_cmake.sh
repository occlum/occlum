#!/bin/bash
set -e

# Skip install if current cmake is newer
CURRENT_VER=`cmake --version | head -n1 | sed 's/[^0-9.]*//g'`
MIN_VER=3.15.5
echo -e "$MIN_VER\n$CURRENT_VER" \ | sort -V | head -n1 | grep -q $MIN_VER && exit 0

rm -rf cmake-3.15.5*
wget https://github.com/Kitware/CMake/releases/download/v3.15.5/cmake-3.15.5.tar.gz && tar xf cmake-3.15.5.tar.gz
cd cmake-3.15.5
./bootstrap
make -j$(nproc)
sudo make install

echo "Install CMake success"
