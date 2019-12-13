#!/bin/sh

install_dir=/usr/local/occlum/x86_64-linux-musl

export PATH=$PATH:$install_dir/bin

cd client

make -j8
if [ $? -ne 0 ]
then
  echo "demo make failed"
  exit 1
fi

./greeter_client
