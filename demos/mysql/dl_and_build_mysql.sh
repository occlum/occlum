#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Start installing dependencies.${NC}"

# Prepare environment
DEPS="libnuma-dev libboost-all-dev"

apt-get update
apt-get install -y ${DEPS}

BOOST="boost_1_77_0"
wget https://boostorg.jfrog.io/artifactory/main/release/1.77.0/source/${BOOST}.tar.bz2
tar --bzip2 -xf ${BOOST}.tar.bz2
pushd ${BOOST}
./bootstrap.sh --prefix=/usr --with-python=python3 &&
./b2 stage -j4 threading=multi link=shared
./b2 install threading=multi link=shared
popd

echo -e "${BLUE}Finish installing dependencies.${NC}"

echo -e "${BLUE}Start building mysql from src.${NC}"

# Download released tarball
VERSION="8.0.31"
TARBALL="mysql-${VERSION}.tar.gz"
wget https://github.com/mysql/mysql-server/archive/refs/tags/${TARBALL}
rm -rf mysql_src && mkdir mysql_src
tar -xf ${TARBALL} -C mysql_src --strip-components 1

# Make modification to
# 1. Disable `times` syscall
patch -s -p0 < apply-mysql-to-occlum.patch

# Build and install
pushd mysql_src
mkdir bld && cd bld

cmake -j$(nproc) .. -DCMAKE_CXX_FLAGS="-fpic -pie" -DCMAKE_C_FLAGS="-fpic -pie"

CC="-fpic -pie" CXX="-fpic -pie" make -j$(nproc)

make install -j$(nproc)
cd ..

echo -e "${BLUE}Finish building mysql from src.${NC}"
popd
