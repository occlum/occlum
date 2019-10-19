# Use Tensorflow Lite with Occlum

This project demonstrates how Occlum enables [Tensorflow Lite](https://www.tensorflow.org/lite) in SGX enclaves.

Step 1: Download Tensorflow, build Tensorflow Lite, and download models
```
./download_and_build_tflite.sh
```
When completed, the resulting Tensorflow can be found in `tensorflow_src` directory, the Tensorflow Lite Model can be found in `models` directory

Step 2.1: To run TensorFlow Lite inference demo in Occlum, run
```
./run_tflite_in_occlum.sh demo
```

Step 2.2: To run TensorFlow Lite inference benchmark in Occlum, run
```
./run_tflite_in_occlum.sh benchmark
```

Step 3.1 (Optional): To run TensorFlow Lite inference demo in Linux, run
```
./run_tflite_in_linux.sh demo
```

Step 3.2 (Optional): To run TensorFlow Lite inference benchmark in Linux, run
```
./run_tflite_in_linux.sh benchmark
```
