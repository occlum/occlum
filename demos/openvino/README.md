# Use OpenVINO Inference Engine in SGX with Occlum

This project demonstrates how Occlum enables [OpenVINO](https://github.com/openvinotoolkit/openvino) in SGX enclaves.

Step 1: Download and build OpenVINO **2023.3.0**
```
./download_and_build_openvino.sh
```
When completed, the resulting OpenVINO can be found in `/usr/local/openvino` directory. OpenMP is used for THREADING.

Step 2: Download the example of OpenVINO models from [open_model_zoo]( https://storage.openvinotoolkit.org/repositories/open_model_zoo)
```
./download_openvino_model.sh
```

Step 3: Run OpenVINO Inference Engine benchmark app inside SGX enclave with Occlum
```
./run_benchmark_on_occlum.sh
```

Step 4 (Optional): Run OpenVINO Inference Engine benchmark app in Linux
```
./openvino_src/bin/intel64/Release/benchmark_app -m ./model/age-gender-recognition-retail-0013.xml
```
