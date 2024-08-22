#!/bin/bash
PREFIX=/usr/local/redis/
set -e
rm -rf $PREFIX

# Build redis
build_redis() {
    rm -rf redis_src && mkdir redis_src
    pushd redis_src
    git clone https://github.com/redis/redis.git .
    git checkout -b 6.0.16  6.0.16
    make -j `getconf _NPROCESSORS_ONLN` BUILD_TLS=yes
    make BUILD_TLS=yes PREFIX=$PREFIX install
    popd
}

# Tell CMake to search for packages in Occlum toolchain's directory only
export PKG_CONFIG_LIBDIR=$PREFIX/lib
build_redis

