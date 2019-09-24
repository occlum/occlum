#!/bin/bash
https_server=simplest_web_server_ssl
set -e

# 1. Init Occlum Workspace
rm -rf occlum_workspace
mkdir occlum_workspace
cd occlum_workspace
occlum init

# 2. Copy files into Occlum Workspace and Build
cp ../mongoose_src/examples/simplest_web_server_ssl/$https_server image/bin
cp -r ../mongoose_src/examples/simplest_web_server_ssl/server.* image
cp /usr/local/occlum/x86_64-linux-musl/lib/libssl.so.1.1 image/lib
cp /usr/local/occlum/x86_64-linux-musl/lib/libcrypto.so.1.1 image/lib
occlum build

# 3. Run https_server
occlum run /bin/$https_server
