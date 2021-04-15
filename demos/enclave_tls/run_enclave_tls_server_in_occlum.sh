#!/bin/bash
set -e

# 1. Init Occlum Workspace
rm -rf occlum_workspace
mkdir occlum_workspace
cd occlum_workspace
occlum init

# 2. Copy files into Occlum Workspace and Build
cp -f /opt/enclave-tls/bin/enclave-tls-server image/bin
cp -f /opt/occlum/glibc/lib/libdl.so.2 image/opt/occlum/glibc/lib
mkdir -p image/opt/enclave-tls
cp -rf /opt/enclave-tls/lib image/opt/enclave-tls
# The following libs are required by libenclave_quote_sgx_ecdsa.so
cp /usr/lib/x86_64-linux-gnu/libsgx_dcap_quoteverify.so.1 image/opt/occlum/glibc/lib
occlum build

# 3. Run enclave_tls_server
occlum run /bin/enclave-tls-server &
