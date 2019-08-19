#!/bin/bash
set -e

show_usage() {
    echo
    echo "Usage: $0 demo/benchmark"
    echo
}

init_workspace() {
    rm -rf occlum_workspace
    mkdir occlum_workspace
    cd occlum_workspace
    occlum init
}

build_occlum() {
    cp ../tensorflow_src/tensorflow/lite/tools/make/gen/linux_x86_64/bin/* image/bin
    cp /usr/local/occlum/x86_64-linux-musl/lib/libz.so.1 image/lib
    cp -r ../models image
    cp -r ../tensorflow_src/tensorflow/lite/examples/label_image/testdata image
    occlum build
}

run_demo() {
    init_workspace
    build_occlum
    occlum run /bin/label_image \
        --tflite_model ./models/mobilenet_v1_1.0_224.tflite \
        --labels ./models/labels.txt \
        --image ./testdata/grace_hopper.bmp
}

run_benchmark() {
    init_workspace
    build_occlum
    occlum run /bin/benchmark_model \
        --graph=./models/mobilenet_v1_1.0_224.tflite \
        --warmup_runs=5
}

bin=$1
case "$bin" in
    demo)
        run_demo
        ;;
    benchmark)
        run_benchmark
        ;;
    *)
        show_usage
esac
