# Use OpenVINO Inference Engine in SGX with Occlum

This project demonstrates how Occlum enables [OpenVINO](https://github.com/opencv/dldt) in SGX enclaves.

Step 1: Install CMake(3.15.5), because OpenVINO depends on a newer version of CMake
```
./install_cmake.sh
```

Step 2: Download OpenVINO and build the Inference Engine, it will also download and install OpenCV
```
./download_and_build_openvino.sh
```
When completed, the resulting OpenVINO can be found in `openvino_src` directory. Threading Building Blocks (TBB) is used by default. To use OpenMP, add option `--threading OMP` when invoking the script above.

Step 3: Download the example of OpenVINO models from [01.org](https://download.01.org/opencv/)
```
./download_openvino_model.sh
```

Step 4: Run OpenVINO Inference Engine benchmark app inside SGX enclave with Occlum
```
./run_benchmark_on_occlum.sh
```

Step 5 (Optional): Run OpenVINO Inference Engine benchmark app in Linux
```
./openvino_src/inference-engine/bin/intel64/Release/benchmark_app -m ./model/age-gender-recognition-retail-0013.xml
```
