#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
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

cmake -j$(nproc) .. \
    -DCMAKE_CXX_FLAGS="-fpic -pie" -DCMAKE_C_FLAGS="-fpic -pie" \
    -DWITH_ARCHIVE_STORAGE_ENGINE=0 \
    -DWITH_EXAMPLE_STORAGE_ENGINE=0 \
    -DWITH_FEDERATED_STORAGE_ENGINE=0 \
    -DDISABLE_PSI_COND=1 \
    -DDISABLE_PSI_DATA_LOCK=1 \
    -DDISABLE_PSI_ERROR=1 \
    -DDISABLE_PSI_FILE=1 \
    -DDISABLE_PSI_IDLE=1 \
    -DDISABLE_PSI_MEMORY=1 \
    -DDISABLE_PSI_METADATA=1 \
    -DDISABLE_PSI_MUTEX=1 \
    -DDISABLE_PSI_PS=1 \
    -DDISABLE_PSI_RWLOCK=1 \
    -DDISABLE_PSI_SOCKET=1 \
    -DDISABLE_PSI_SP=1 \
    -DDISABLE_PSI_STAGE=0 \
    -DDISABLE_PSI_STATEMENT=1 \
    -DDISABLE_PSI_STATEMENT_DIGEST=1 \
    -DDISABLE_PSI_TABLE=1 \
    -DDISABLE_PSI_THREAD=0 \
    -DDISABLE_PSI_TRANSACTION=1 \
    -DWITH_MYSQLX=0 \
    -DWITH_NDB_JAVA=0 \
    -DWITH_RAPID=0 \
    -DWITH_ROUTER=0 \
    -DWITH_UNIT_TESTS=0

make -j4
make install -j$(nproc)
cd ..

echo -e "${BLUE}Finish building mysql from src.${NC}"
popd
