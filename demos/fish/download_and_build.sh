#!/bin/bash
set -e

# download and install ncurses
git clone -b v6.1 --depth 1 https://github.com/mirror/ncurses.git
cd ncurses
CC=occlum-gcc CXX=occlum-g++ CFLAGS="-O2 -fPIC" CXXFLAGS="-O2 -fPIC" LDFLAGS="-pie"  \
./configure --without-shared --without-cxx-shared --prefix=/usr/local/occlum --enable-overwrite
make && make install
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
-DCMAKE_CXX_FLAGS="-I/usr/local/occlum/include -fpic -pie"
make
cd ../../

# download and build busybox
git clone -b 1_31_1 --depth 1 https://github.com/mirror/busybox.git
cd busybox
CROSS_COMPILE=/opt/occlum/toolchains/gcc/bin/occlum-
make CROSS_COMPILE="$CROSS_COMPILE" defconfig
cp ../.config .
make CROSS_COMPILE="$CROSS_COMPILE"
