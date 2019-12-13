#!/bin/bash
install_dir=/usr/local/occlum/x86_64-linux-musl

export PATH=$PATH:$install_dir/bin

cd client

make -j8
if [ $? -ne 0 ]
then
  echo "demo make failed"
  exit 1
fi

rm -rf occlum_context
mkdir occlum_context
cd occlum_context

occlum init
if [ $? -ne 0 ]
then
  echo "occlum init failed"
  exit 1
fi

mkdir -p image/etc
cp /etc/resolv.conf image/etc
cp ../greeter_client image/bin
cp $install_dir/lib/libprotobuf.so.3.10.0.0 image/lib
cp $install_dir/lib/libcares.so.2 image/lib
cp $install_dir/lib/libz.so.1 image/lib
if [ $? -ne 0 ]
then
  echo "libraries copied failed"
  exit 1
fi

occlum build
if [ $? -ne 0 ]
then
  echo "occlum build failed"
  exit 1
fi

occlum run /bin/greeter_client
