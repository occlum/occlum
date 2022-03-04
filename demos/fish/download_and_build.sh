#!/bin/bash
set -e

rm -rf ncurses
# download and install ncurses
git clone -b v6.1 --depth 1 https://github.com/mirror/ncurses.git
cd ncurses
CC=occlum-gcc CXX=occlum-g++ CFLAGS="-O2 -fPIC" CXXFLAGS="-O2 -fPIC" LDFLAGS="-pie"  \
./configure --without-shared --without-cxx-shared --prefix=/usr/local/occlum --enable-overwrite
make -j$(nproc) && make install
cd ..

# download and build FISH
git clone -b 3.1.2 --depth 1 https://github.com/fish-shell/fish-shell.git
cd fish-shell
git apply ../fish.patch
mkdir build && cd build
cmake ../  -DCMAKE_BUILD_TYPE=Debug -DCURSES_LIBRARY=/opt/occlum/toolchains/gcc/lib/libcurses.a \
-DCMAKE_C_COMPILER=occlum-gcc -DCMAKE_CXX_COMPILER=occlum-g++ \
-DCMAKE_CXX_COMPILER_AR=/usr/local/occlum/bin/occlum-ar \
-DCMAKE_CXX_COMPILER_RANLIB=/usr/local/occlum/bin/occlum-ranlib \
-DCMAKE_C_COMPILER_AR=/usr/local/occlum/bin/occlum-ar \
-DCMAKE_C_COMPILER_RANLIB=/usr/local/occlum/bin/occlum-ranlib \
-DCMAKE_LINKER=/usr/local/occlum/bin/occlum-ld -DCMAKE_C_FLAGS="-I/usr/local/occlum/include -fpic -pie" \
-DCMAKE_CXX_FLAGS="-I/usr/local/occlum/include -fpic -pie" \
-DIntl_INCLUDE_DIR=/usr/local/occlum/x86_64-linux-musl/include
make -j$(nproc)
cd ../../

