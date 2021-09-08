#!/bin/bash
set -e

branch=master

rm -rf inclavare-containers
git clone -b $branch https://github.com/alibaba/inclavare-containers

cd inclavare-containers && git apply ../0001-Consider-SGX_QL_QV_RESULT_OUT_OF_DATE-as-success.patch
cd rats-tls
cmake -DRATS_TLS_BUILD_MODE="occlum" -DBUILD_SAMPLES=on -H. -Bbuild
make -C build install
