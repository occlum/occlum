#!/bin/sh

INSTALL_DIR=/usr/local/occlum/x86_64-linux-musl

export PATH=$PATH:$INSTALL_DIR/bin

cd client

make -j$(nproc)
if [ $? -ne 0 ]
then
  echo "demo make failed"
  exit 1
fi

./greeter_client
