#!/bin/sh
#copyright@antfinancial:adopted from a script written by geding

git clone http://github.com/openssl/openssl
cd openssl
git checkout tags/OpenSSL_1_1_1
CC=occlum-gcc ./config \
    --prefix=/usr/local/occlum/x86_64-linux-musl \
    --openssldir=/usr/local/occlum/x86_64-linux-musl/ssl \
    --with-rand-seed=rdcpu \
    no-async no-zlib
if [ $? -ne 0 ]
then
  echo "./config command failed."
  exit 1
fi
make -j$(nproc)
if [ $? -ne 0 ]
then
  echo "make command failed."
  exit 1
fi
make install
if [ $? -ne 0 ]
then
  echo "make install command failed."
  exit 1
fi

echo "build and install openssl success!"
