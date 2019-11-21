#!/bin/bash
set -e

# 1. Download and install OpenSSL 1.1.1
mkdir -p deps/openssl
pushd deps/openssl
git clone https://github.com/openssl/openssl .
git checkout tags/OpenSSL_1_1_1 -b OpenSSL_1_1_1
CC=occlum-gcc ./config \
  --prefix=/usr/local/occlum/x86_64-linux-musl \
  --openssldir=/usr/local/occlum/ssl \
  --with-rand-seed=rdcpu \
  no-zlib no-async no-tests
make -j
sudo make install
popd

# 2. Download Mongoose 6.15
mkdir -p mongoose_src
pushd mongoose_src
git clone https://github.com/cesanta/mongoose .
git checkout tags/6.15 -b 6.15
popd

# 3. Build the https server example in mongoose
pushd mongoose_src/examples/simplest_web_server_ssl
CC=occlum-gcc CFLAGS_EXTRA="-Wno-format-truncation" make
popd
