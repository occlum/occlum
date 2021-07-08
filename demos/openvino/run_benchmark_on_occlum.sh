#!/bin/bash
benchmark=benchmark_app
inference_bin=openvino_src/bin/intel64/Release
occlum_lib=/usr/local/occlum/x86_64-linux-musl/lib
set -e

# 1. Init Occlum Workspace
rm -rf occlum_instance
mkdir occlum_instance
cd occlum_instance
occlum init
cpu_cc=`cat /proc/cpuinfo | grep processor | wc -l`
#new_json="$(jq '.resource_limits.user_space_size = "4GB" |
#                .resource_limits.kernel_space_heap_size = "128MB" |
#                .resource_limits.kernel_space_stack_size = "16MB" |
#                .resource_limits.max_num_of_threads = 128 |
#                .process.default_mmap_size = "1024MB" |
#                .process.default_stack_size = "8MB" |
#                .process.default_heap_size = "32MB" |
#                .metadata.debuggable = false ' Occlum.json)" && \
new_json="$(jq '.resource_limits.user_space_size = "320MB" |
                .process.default_mmap_size = "256MB"' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy files into Occlum Workspace and Build
cp ../$inference_bin/$benchmark image/bin
cp ../$inference_bin/lib/libinference_engine.so image/lib
cp ../$inference_bin/lib/libinference_engine_c_api.so image/lib
cp ../$inference_bin/lib/libformat_reader.so image/lib
cp ../$inference_bin/lib/libinference_engine_transformations.so image/lib
cp ../$inference_bin/lib/libngraph.so image/lib
cp ../$inference_bin/lib/libinference_engine_ir_v7_reader.so image/lib
cp ../$inference_bin/lib/libinference_engine_ir_reader.so image/lib
cp ../$inference_bin/lib/libMKLDNNPlugin.so image/lib
cp ../$inference_bin/lib/libinference_engine_legacy.so image/lib
cp ../$inference_bin/lib/libinference_engine_lp_transformations.so image/lib
cp ../$inference_bin/lib/plugins.xml image/lib
cp $occlum_lib/libopencv_imgcodecs.so.4.1 image/lib
cp $occlum_lib/libopencv_imgproc.so.4.1 image/lib
cp $occlum_lib/libopencv_core.so.4.1 image/lib
cp $occlum_lib/libopencv_videoio.so.4.1 image/lib
cp $occlum_lib/libz.so.1 image/lib
[ -e $occlum_lib/libtbb.so.2 ] && cp $occlum_lib/libtbb.so.2 image/lib
[ -e $occlum_lib/libtbbmalloc.so.2 ] && cp $occlum_lib/libtbbmalloc.so.2 image/lib
mkdir image/model
cp -r ../model/* image/model
occlum build

# 3. Run benchmark
occlum run /bin/$benchmark -m /model/age-gender-recognition-retail-0013.xml
