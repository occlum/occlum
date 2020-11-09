#!/bin/bash
set -e
PREFIX=/opt/occlum/toolchains/gcc/x86_64-linux-musl
export PATH="/usr/local/occlum/bin:$PATH"

rm -rf zlib && mkdir -p zlib
pushd zlib
git clone https://github.com/madler/zlib .
git checkout -b v1.2.11 tags/v1.2.11
CC=occlum-gcc CXX=occlum-g++ ./configure --prefix=$PREFIX
make
sudo make install
popd
