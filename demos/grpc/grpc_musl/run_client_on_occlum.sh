#!/bin/bash
INSTALL_DIR=/usr/local/occlum/x86_64-linux-musl

export PATH=$INSTALL_DIR/bin:$PATH

cd client

make -j$(nproc)
if [ $? -ne 0 ]
then
  echo "demo make failed"
  exit 1
fi

rm -rf occlum_instance
mkdir occlum_instance
cd occlum_instance

occlum init
if [ $? -ne 0 ]
then
  echo "occlum init failed"
  exit 1
fi

rm -rf image && \
copy_bom -f ../../grpc_client.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

if [ $? -ne 0 ]
then
  echo "occlum build failed"
  exit 1
fi

occlum run /bin/greeter_client
