#! /bin/bash
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )

TAG=20210927
LTP_INSTALL_DIR=${SCRIPT_DIR}/ltp_install/ltp

rm -rf ltp
rm -rf $LTP_INSTALL_DIR && mkdir -p ${LTP_INSTALL_DIR}
git clone -b $TAG https://github.com/linux-test-project/ltp.git

pushd ltp
# Apply patch to support running ltp in Occlum
git apply ../0001-Make-it-work-on-Occlum.patch
make autotools
./configure --prefix=${LTP_INSTALL_DIR}
make -j && make install

popd