#!/bin/bash
SRC_DIR=/tmp/glibc/glibc
BUILD_DIR=/tmp/glibc/glibc_build
INSTALL_DIR=/opt/occlum/glibc

# GCC 9/10 introduces many new checkings and will cause the build to fail.
if [ "$(gcc -dumpversion)" = "9" -o "$(gcc -dumpversion)" = "10" ]; then
    EXTRA_CFLAGS=-fcommon
    EXTRA_CONFIG_OPTION="--disable-werror"
fi

# Exit if any command fails
set -e

# Clean previous build and installation if any
rm -rf ${SRC_DIR}
rm -rf ${BUILD_DIR}
rm -rf ${INSTALL_DIR}

mkdir -p ${SRC_DIR}
cd ${SRC_DIR}
# Download glibc
git clone -b occlum-glibc-2.31 https://github.com/occlum/glibc .

mkdir -p ${BUILD_DIR}
cd ${BUILD_DIR}
# Build and install glibc
unset LD_LIBRARY_PATH
CFLAGS="-O2 -g ${EXTRA_CFLAGS}" ${SRC_DIR}/configure \
  --prefix=${INSTALL_DIR} --with-tls --without-selinux \
  --enable-stack-protector=strong --disable-nscd --disable-crypt \
  ${EXTRA_CONFIG_OPTION}
make -j$(nproc)
make install
