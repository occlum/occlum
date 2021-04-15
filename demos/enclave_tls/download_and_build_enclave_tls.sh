#!/bin/bash
set -e

# Download and Build Enclave Tls server
mkdir -p enclave_tls_src
pushd enclave_tls_src
git clone https://github.com/alibaba/inclavare-containers
cd inclavare-containers/enclave-tls && make OCCLUM=1 && make install
popd
