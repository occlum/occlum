#!/bin/bash
#copyright@antfinancial:adopted from a script written by geding
set -e

git clone -b OpenSSL_1_1_1 --depth 1 http://github.com/openssl/openssl
cd openssl
CC=occlum-gcc ./config \
    --prefix=/usr/local/occlum/x86_64-linux-musl \
    --openssldir=/usr/local/occlum/x86_64-linux-musl/ssl \
    --with-rand-seed=rdcpu \
    no-async no-zlib

make -j$(nproc)
make install

echo "build and install openssl success!"
