#!/bin/bash
occlum_glibc=/opt/occlum/glibc/lib/
host_libs=/lib/x86_64-linux-gnu/
set -e
ssl_config_file=/bin/ssl_configure/ssl.cfg
model_name=resnet50-v15-fp32
enable_batching=false
rest_api_num_threads=8
session_parallelism=0
parallel_num_threads=2


unset http_proxy https_proxy


# 1. Init Occlum Workspace
rm -rf occlum_instance
mkdir occlum_instance
cd occlum_instance
occlum init
yq '.resource_limits.user_space_size.init = "7000MB" |
    .resource_limits.kernel_space_heap_size.init="1024MB" |
    .process.default_heap_size = "128MB" |
    .env.default = [ "OMP_NUM_THREADS=8", "KMP_AFFINITY=verbose,granularity=fine,compact,1,0", "KMP_BLOCKTIME=20", "MKL_NUM_THREADS=8"]' \
    -i Occlum.yaml

# 2. Copy files into Occlum Workspace and Build
rm -rf image
copy_bom -f ../tensorflow_serving.yaml --root image --include-dir /opt/occlum/etc/template

#occlum build
occlum build
# 3. Run benchmark
occlum run /bin/tensorflow_model_server \
    --model_name=${model_name} \
    --model_base_path=/model/${model_name} \
    --port=8500 \
    --rest_api_port=8501 \
    --enable_model_warmup=true \
    --flush_filesystem_caches=false \
    --enable_batching=${enable_batching} \
    --rest_api_num_threads=${rest_api_num_threads} \
    --tensorflow_session_parallelism=${session_parallelism} \
    --tensorflow_intra_op_parallelism=${parallel_num_threads} \
    --tensorflow_inter_op_parallelism=${parallel_num_threads} \
    --ssl_config_file=${ssl_config_file} \
	&
