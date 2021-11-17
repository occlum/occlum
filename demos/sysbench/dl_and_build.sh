#! /bin/bash
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )

TAG=1.0.20
SYSBENCH_ISNTALL_DIR=${SCRIPT_DIR}/sysbench-install

rm -rf sysbench-* $TAG.tar.gz*
wget https://github.com/akopytov/sysbench/archive/refs/tags/$TAG.tar.gz
tar zxvf $TAG.tar.gz

pushd sysbench-$TAG
./autogen.sh
./configure --without-mysql --prefix=${SYSBENCH_ISNTALL_DIR}
make -j
make install
popd

