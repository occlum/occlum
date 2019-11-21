#!/bin/bash
https_server=simplest_web_server_ssl
set -e

# 1. Copy files
cp -f mongoose_src/examples/simplest_web_server_ssl/$https_server .
cp -rf mongoose_src/examples/simplest_web_server_ssl/server.* .

# 2. Run https_server
./$https_server
