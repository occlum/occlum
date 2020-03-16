#!/bin/bash
benchmark=benchmark_app
inference_bin=openvino_src/inference-engine/bin/intel64/Release
occlum_lib=/usr/local/occlum/x86_64-linux-musl/lib
set -e

# 1. Init Occlum Workspace
rm -rf occlum_context
mkdir occlum_context
cd occlum_context
occlum init
jq '.vm.user_space_size = "320MB"' Occlum.json > temp_Occlum.json
jq '.process.default_mmap_size = "256MB"' temp_Occlum.json > Occlum.json

# 2. Copy files into Occlum Workspace and Build
cp ../$inference_bin/$benchmark image/bin
cp ../$inference_bin/lib/libinference_engine.so image/lib
cp ../$inference_bin/lib/libformat_reader.so image/lib
cp ../$inference_bin/lib/libcpu_extension.so image/lib
cp ../$inference_bin/lib/libHeteroPlugin.so image/lib
cp ../$inference_bin/lib/libMKLDNNPlugin.so image/lib
cp ../$inference_bin/lib/plugins.xml image/lib
cp $occlum_lib/libopencv_imgcodecs.so.4.1 image/lib
cp $occlum_lib/libopencv_imgproc.so.4.1 image/lib
cp $occlum_lib/libopencv_core.so.4.1 image/lib
cp $occlum_lib/libopencv_videoio.so.4.1 image/lib
cp $occlum_lib/libz.so.1 image/lib
[ -e $occlum_lib/libtbb.so ] && cp $occlum_lib/libtbb.so image/lib
[ -e $occlum_lib/libtbbmalloc.so ] && cp $occlum_lib/libtbbmalloc.so image/lib
mkdir image/proc
cp /proc/cpuinfo image/proc
mkdir image/model
cp -r ../model/* image/model
occlum build

# 3. Run benchmark
occlum run /bin/$benchmark -m /model/age-gender-recognition-retail-0013.xml
