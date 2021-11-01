#! /bin/bash
set -e
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
INSTALL_DIR=/opt/occlum/toolchains/bash

bash_source="https://github.com/occlum/bash.git"
bash_branch="bash_5.1_for_occlum"

rm -rf ${INSTALL_DIR}

# Download bash source
git clone -b ${bash_branch} ${bash_source}

echo "Building bash with musl-gcc (occlum-gcc) ..."
cd bash
CC="occlum-gcc -fPIE -pie" CXX="occlum-g++ -fPIE -pie" ./configure --without-bash-malloc
make clean
make -j

mkdir -p ${INSTALL_DIR}/musl
cp bash ${INSTALL_DIR}/musl/

# Restore code
make clean
git clean -dxf
git reset HEAD --hard

echo "Building bash with gcc ..."
CC="gcc -fPIE -pie" CXX="g++ -fPIE -pie" ./configure
make -j

mkdir -p ${INSTALL_DIR}/glibc
cp bash ${INSTALL_DIR}/glibc/

