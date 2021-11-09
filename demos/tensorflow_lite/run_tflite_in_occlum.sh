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
    rm -rf image
    copy_bom -f ../tflite.yaml --root image --include-dir /opt/occlum/etc/template;
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
