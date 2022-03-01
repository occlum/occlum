#!/bin/bash
set -e
cd ~
wget https://github.com/Kitware/CMake/releases/download/v3.15.5/cmake-3.15.5.tar.gz && tar xf cmake-3.15.5.tar.gz
cd cmake-3.15.5
./bootstrap
make -j$(nproc)
sudo make install
