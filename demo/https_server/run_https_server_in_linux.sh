#!/bin/bash
export LD_LIBRARY_PATH=/usr/local/occlum/x86_64-linux-musl/lib:$LD_LIBRARY_PATH
https_server=simplest_web_server_ssl
set -e

# 1. Copy files
cp -f mongoose_src/examples/simplest_web_server_ssl/$https_server .
cp -rf mongoose_src/examples/simplest_web_server_ssl/server.* .

# 2. Run https_server
./$https_server
