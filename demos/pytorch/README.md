# Use PyTorch with Python and Occlum

This project demonstrates how Occlum enables _unmodified_ [PyTorch](https://pytorch.org/) programs running in SGX enclaves, on the basis of _unmodified_ [Python](https://www.python.org).

## Sample Code: Linear model

Use the nn package to define our model as a sequence of layers. nn.Sequential is a Module which contains other Modules, and applies them in sequence to produce its output. Each Linear Module computes output from input using a linear function, and holds internal Tensors for its weight and bias.

## How to Run

This tutorial is written under the assumption that you have Docker installed and use Occlum in a Docker container.

Occlum is compatible with glibc-supported Python, we employ miniconda as python installation tool. You can import PyTorch packages using conda. Here, miniconda is automatically installed by install_python_with_conda.sh script, the required python and PyTorch packages for this project are also loaded by this script. Here, we take occlum/occlum:0.23.0-ubuntu18.04 as example.

Step 1 (on the host): Start an Occlum container
```
docker pull occlum/occlum:0.23.0-ubuntu18.04
docker run -it --name=pythonDemo --device /dev/sgx/enclave occlum/occlum:0.23.0-ubuntu18.04 bash
```

Step 2 (in the Occlum container): Download miniconda and install python to prefix position.
```
cd /root/demos/pytorch
bash ./install_python_with_conda.sh
```

Step 3 (in the Occlum container): Run the sample code on Occlum
```
cd /root/demos/pytorch
bash ./run_pytorch_on_occlum.sh
```
