#!/bin/bash
set -e

# 1. Init Occlum Workspace
rm -rf occlum_workspace
mkdir occlum_workspace
cd occlum_workspace
occlum init

# 2. Copy files into Occlum Workspace and Build
rm -rf image && \
copy_bom -f ../enclave_tls.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# 3. Run enclave_tls_server
occlum run /bin/enclave-tls-server &
