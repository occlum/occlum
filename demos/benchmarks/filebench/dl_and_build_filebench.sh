#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Start building filebench from tarball.${NC}"

# Download release tarball
VERSION="1.5-alpha3"
TARBALL="filebench-${VERSION}.tar.gz"
rm -f ${TARBALL}
wget https://github.com/filebench/filebench/releases/download/${VERSION}/${TARBALL}
rm -rf filebench && mkdir filebench
tar -zxvf filebench-${VERSION}.tar.gz -C filebench --strip-components 1
pushd filebench

./configure
popd

# Make modification to
# 1. Replace fork to vfork
# 2. Prepare shared memory region for child processes
# 3. Disable SYSV semaphores
patch -s -p0 < apply-filebench-to-occlum.patch

pushd filebench
# Build and install filebench tool
make -j$(nproc)
make install
echo -e "${BLUE}Finish building filebench from tarball.${NC}"

popd
