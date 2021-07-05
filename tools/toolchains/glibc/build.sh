#!/bin/bash
SRC_DIR=/tmp/glibc/glibc
BUILD_DIR=/tmp/glibc/glibc_build
INSTALL_DIR=/opt/occlum/glibc

# Exit if any command fails
set -e
GCCVERSION=`gcc -dumpversion | cut -f1 -d.`
echo "GCCVERSION=$GCCVERSION"
if [[ $GCCVERSION > 8 ]] ; then
   EXTRA_FLAG=--disable-werror
fi
echo "EXTRA_FLAG=$EXTRA_FLAG"
# Clean previous build and installation if any
rm -rf ${SRC_DIR}
rm -rf ${BUILD_DIR}
rm -rf ${INSTALL_DIR}

mkdir -p ${SRC_DIR}
cd ${SRC_DIR}
# Download glibc
git clone -b occlum-glibc-2.27 https://github.com/occlum/glibc .

mkdir -p ${BUILD_DIR}
cd ${BUILD_DIR}
# Build and install glibc
unset LD_LIBRARY_PATH
echo "CFLAGS="-O2 -g" ${SRC_DIR}/configure \
  --prefix=${INSTALL_DIR} --with-tls --without-selinux \
  --enable-stack-protector=strong --disable-nscd $EXTRA_FLAG"
CFLAGS="-O2 -g" ${SRC_DIR}/configure \
  --prefix=${INSTALL_DIR} --with-tls --without-selinux \
  --enable-stack-protector=strong --disable-nscd $EXTRA_FLAG
make
make install
