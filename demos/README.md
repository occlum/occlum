# Demos

This directory contains sample projects that demonstrate how Occlum can be used to build and run user applications.

## Toolchain demos

This set of demos shows how the Occlum toolchain can be used with different build tools.

* [hello_c](hello_c/): A sample C project built with Makefile/CMake.
* [hello_cc](hello_cc/): A sample C++ project built with Makefile/CMake.
* [hello_bazel](hello_bazel/): A sample C++ project built with [Bazel](https://bazel.build).

## Application demos

This set of demos shows how real-world apps can be easily run inside SGX enclaves with Occlum.

* [fish](fish/): A demo of [FISH](https://fishshell.com) shell script.
* [flink](flink/): A demo of [Apache Flink](https://flink.apache.org).
* [font](font/font_support_for_java): A demo of supporting font with Java.
* [https_server](https_server/): A HTTPS file server based on [Mongoose Embedded Web Server Library](https://github.com/cesanta/mongoose).
* [grpc](grpc/): A client and server communicating through [gRPC](https://grpc.io).
* [openvino](openvino/) A benchmark of [OpenVINO Inference Engine](https://docs.openvinotoolkit.org/2019_R3/_docs_IE_DG_inference_engine_intro.html).
* [redis](redis/): A demo of [redis](https://redis.io).
* [sqlite](sqlite/) A demo of [SQLite](https://www.sqlite.org) SQL database engine.
* [tensorflow_lite](tensorflow_lite/): A demo and benchmark of [Tensorflow Lite](https://www.tensorflow.org/lite) inference engine.
* [xgboost](xgboost/): A demo of [XGBoost](https://xgboost.readthedocs.io/en/latest).

## Programming language demos

This set of demos shows how apps written with popular programming languages can be run inside SGX enclaves with Occlum.

* [golang](golang/): A collection of [Golang](https://golang.org) program demos.
* [java](java/): A demo of [Java](https://openjdk.java.net) program.
* [python](python/) A demo of [Python](https://www.python.org) program.
* [rust](rust/) A demo of [Rust](https://www.rust-lang.org) program.

## Other demos

* [embedded_mode](embedded_mode/): A cross-enclave memory throughput benchmark enabled by the embedded mode of Occlum.
* [gdb_support](gdb_support/): This demo explains the technical detail of GDB support and demonstrates how to debug an app running upon Occlum with GDB.
* [local_attestation](local_attestation/): This project demonstrates how an app running upon Occlum can perform SGX local attestation.
* [remote_attestation](remote_attestation/): This project demonstrates how an app running upon Occlum can perform SGX remote attestation.
