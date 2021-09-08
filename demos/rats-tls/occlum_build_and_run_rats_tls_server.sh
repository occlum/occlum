#!/bin/bash
set -e

install_sample_dir=/usr/share/rats-tls/samples
install_lib_dir=/usr/local/lib/rats-tls

rm -rf occlum_server
occlum new occlum_server
cd occlum_server

# Copy libs/files to prepare occlum image
cp ${install_sample_dir}/rats-tls-server image/bin
cp /lib/x86_64-linux-gnu/libdl.so.2 image/opt/occlum/glibc/lib
cp /usr/lib/x86_64-linux-gnu/libssl.so.1.1 image/opt/occlum/glibc/lib
cp /usr/lib/x86_64-linux-gnu/libcrypto.so.1.1 image/opt/occlum/glibc/lib
mkdir -p image/usr/local/lib
cp -rf ${install_lib_dir} image/usr/local/lib/

occlum build

echo "Run the rats-tls server on background ..."
occlum run /bin/rats-tls-server -m &
