#!/bin/bash
set -e

show_usage() {
    echo
    echo "Usage: $0 demo/benchmark"
    echo
}

copy_files() {
    cp -f tensorflow_src/tensorflow/lite/tools/make/gen/linux_x86_64/bin/* .
    cp -rf tensorflow_src/tensorflow/lite/examples/label_image/testdata .
}

run_demo() {
    copy_files
    ./label_image \
        --tflite_model ./models/mobilenet_v1_1.0_224.tflite \
        --labels ./models/labels.txt \
        --image ./testdata/grace_hopper.bmp
}

run_benchmark() {
    copy_files
    ./benchmark_model \
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
