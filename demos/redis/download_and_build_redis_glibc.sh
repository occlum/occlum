#!/bin/bash
PREFIX=/usr/local/redis/
OCCLUM_LINKER=/opt/occlum/glibc/lib/ld-linux-x86-64.so.2
set -e
rm -rf $PREFIX


build_openssl() {
    # 1. Download and install OpenSSL 1.1.1
    rm -rf deps && mkdir -p deps/openssl
    pushd deps/openssl
    git clone https://github.com/openssl/openssl .
    git checkout tags/OpenSSL_1_1_1 -b OpenSSL_1_1_1
    ./config \
        --openssldir=/usr/local/occlum/ssl \
        --with-rand-seed=rdcpu \
        no-zlib no-async no-tests
    make -j `getconf _NPROCESSORS_ONLN`
    sudo make install
    popd
}

# Build redis
build_redis() {
    rm -rf redis_src && mkdir redis_src
    pushd redis_src
    git clone https://github.com/redis/redis.git .
    git checkout -b 6.0.9  6.0.9
    make -j `getconf _NPROCESSORS_ONLN` BUILD_TLS=yes
    make  PREFIX=$PREFIX install
    popd
}

# Tell CMake to search for packages in Occlum toolchain's directory only
export PKG_CONFIG_LIBDIR=$PREFIX/lib
build_openssl
build_redis

