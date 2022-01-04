#!/bin/bash
#copyright@antfinancial:adopted from a script written by geding
set -e

git clone http://github.com/openssl/openssl
cd openssl
git checkout tags/OpenSSL_1_1_1
CC=occlum-gcc ./config \
    --prefix=/usr/local/occlum/x86_64-linux-musl \
    --openssldir=/usr/local/occlum/x86_64-linux-musl/ssl \
    --with-rand-seed=rdcpu \
    no-async no-zlib

make -j$(nproc)
make install

echo "build and install openssl success!"
