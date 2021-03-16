#!/bin/bash
redis_dir=/usr/local/redis/
occlum_glibc=/opt/occlum/glibc/lib/
set -e

# 1. Init Occlum Workspace
rm -rf occlum_instance
occlum new occlum_instance
cd occlum_instance
new_json="$(jq '.resource_limits.user_space_size = "320MB" |
                .process.default_mmap_size = "256MB"' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy files into Occlum Workspace and Build
cp $redis_dir/bin/redis* image/bin
cp /usr/local/bin/openssl* image/bin
cp /usr/local/lib/libssl* image/$occlum_glibc
cp /usr/local/lib/libcrypto* image/$occlum_glibc
cp $occlum_glibc/libdl.so.2 image/$occlum_glibc
cp $occlum_glibc/librt.so.1 image/$occlum_glibc
cp $occlum_glibc/libm.so.6 image/$occlum_glibc
#occlum build
occlum build
# 3. Run redis server
occlum run /bin/redis-server --save "" --appendonly no &
