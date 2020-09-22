#!/bin/bash
set -e
out_dir=$PWD/bin



# 1. Init Occlum Workspace
rm -rf occlum_instance && mkdir occlum_instance
cd occlum_instance
occlum init
new_json="$(jq '.resource_limits.user_space_size = "4096MB" |
	        .resource_limits.max_num_of_threads = 96 |
                .process.default_mmap_size = "300MB"' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy program into Occlum Workspace and build
cp ${out_dir}/* image/bin
mkdir -p image/etc
cp /etc/hosts image/etc
occlum build
# Following runs one benchmark
port=50051
echo "================================================================================"
echo "gRPC Server"
# Launch the server in background
occlum run /bin/server --port=${port} --test_name="Server_gRPC"&


