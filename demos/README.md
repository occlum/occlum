# Demos

This directory contains sample projects that demonstrate how Occlum can be used to build and run user applications.

## Toolchain demos

This set of demos shows how the Occlum toolchain can be used with different build tools.

* `hello_c/`: A sample C project built with Makefile/CMake.
* `hello_cc/`: A sample C++ project built with Makefile/CMake.
* `hello_bazel/`: A sample C++ project built with [Bazel](https://bazel.build).

## Application demos

This set of demos shows how real-world apps can be easily run inside SGX enclaves with Occlum.

* `https_server/`: A HTTPS file server based on [Mongoose Embedded Web Server Library](https://github.com/cesanta/mongoose).
* `grpc/`: A client and server communicating through [gRPC](https://grpc.io/).
* `openvino/` A benchmark of [OpenVINO Inference Engine](https://docs.openvinotoolkit.org/2019_R3/_docs_IE_DG_inference_engine_intro.html).
* `python` A demo of [Python](https://www.python.org).
* `tensorflow_lite/`: A demo and benchmark of [Tensorflow Lite](https://www.tensorflow.org/lite) inference engine.
* `xgboost/`: A demo of [XGBoost](https://xgboost.readthedocs.io/en/latest/).

## Other demos

* `remote_attestation/`: This project demonstrates how an app running upon Occlum can perform SGX remote attestation.
* `embedded_mode/`: A cross-enclave memory throughput benchmark enabled by the embedded mode of Occlum.
