# Use PaddlePaddle with Python and Occlum

This project demonstrates how Occlum enables _unmodified_
[PaddlePaddle](https://www.paddlepaddle.org.cn/) programs running in SGX
enclaves, on the basis of _unmodified_ [Python](https://www.python.org). The
workload is a primary AI task from [PaddlePaddle Quick
Start](https://www.paddlepaddle.org.cn/documentation/docs/zh/guides/beginner/quick_start_cn.html).
The source code of the workload resides in `demo.py`.

## How to Run

This tutorial is written under the assumption that you have Docker installed and use Occlum in a Docker container.

Occlum is compatible with glibc-supported Python, we employ miniconda as python installation tool. You can import paddle packages using conda. Here, miniconda is automatically installed by install_python_with_conda.sh script, the required python and paddle packages for this project are also loaded by this script.

Step 1 (on the host): Start an Occlum container
```
docker pull occlum/occlum:latest-ubuntu20.04
docker run -it --name=pythonDemo --device /dev/sgx/enclave occlum/occlum:latest-ubuntu20.04 bash
```

Step 2 (in the Occlum container): Download miniconda and install python to prefix position.
```
cd /root/demos/paddlepaddle
bash ./install_python_with_conda.sh
```

Step 3 (in the Occlum container): Run the sample code on Occlum
```
cd /root/demos/paddlepaddle
bash ./run_paddlepaddle_on_occlum.sh
```
