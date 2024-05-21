# Use Python with Occlum

This project demonstrates how Occlum enables _unmodified_ [Python](https://www.python.org) program [`Catboost`](https://github.com/catboost/catboost) running in SGX enclaves, which is based on glibc.

## Sample Code: CSV Processing in Python

To make the sample code more realistic, we choose to use Catboost "titanic" dataset with code from [main tutorial](https://github.com/catboost/tutorials/blob/master/python_tutorial.ipynb). Dataset was downloaded and stored in datasets/titanic folder, load into code directly by Pandas.
All example code can be found [here](catboost_demo.py).

## How to Run

This tutorial is written under the assumption that you have Docker installed and use Occlum in a Docker container.

Occlum is compatible with glibc-supported Python, we employ miniconda as python installation tool. You can import any python dependencies using conda. Here, miniconda is automatically installed by install_python_and_deps_with_conda.sh script, the required python and related dependencies for this project are also loaded by this script. Here, we take occlum/occlum:latest-ubuntu20.04 as example.

Step 1 (on the host): Start an Occlum container
```
docker pull occlum/occlum:latest-ubuntu20.04
docker run -it --name=pythonCatboostDemo --device /dev/sgx/enclave occlum/occlum:latest-ubuntu20.04 /bin/bash
```

Step 2 (on the host): Download miniconda and install Python with Catboost and deps to prefix position.
```
cd /root/occlum/demos/catboost
bash ./install_python_and_deps_with_conda.sh
```

Step 3 (on the host): Run the sample code on Occlum
```
cd /root/occlum/demos/catboost
bash ./run_catboost_on_occlum.sh
```
It will process CSV data files and generate output log to stdout and to a file (`output.log`) in `./occlum_instance`.
