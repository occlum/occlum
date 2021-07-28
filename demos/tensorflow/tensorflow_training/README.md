# Use TensorFlow with Python and Occlum

This project demonstrates how Occlum enables _unmodified_ [TensorFlow](https://www.tensorflow.org/) programs running in SGX enclaves, on the basis of _unmodified_ [Python](https://www.python.org). Actually, we have tested various _unmodified_ [TensorFlow Benchmarks](https://github.com/tensorflow/benchmarks) on occlum.

## Sample Code: neural network model

This short introduction uses Keras to:

Build a neural network that classifies MNIST handwritten digit images.
Train this neural network.
And, finally, evaluate the accuracy of the model.

## How to Run

This tutorial is written under the assumption that you have Docker installed and use Occlum in a Docker container.

Occlum is compatible with glibc-supported Python, we employ miniconda as python installation tool. You can import TensorFlow packages using conda. Here, miniconda is automatically installed by install_python_with_conda.sh script, the required python and TensorFlow package and MNIST dataset for this project are also loaded by this script. 

Step 1 (on the host): Start an Occlum container
```
docker run -it --name=tensorflowDemo --device /dev/sgx occlum/occlum:[version]-ubuntu18.04 bash
```

Step 2 (on the host): Download miniconda and install python
```
cd /root/occlum/demos/tensorflow/tensorflow_training
bash ./install_python_with_conda.sh
```

Step 3 (on the host): Run the sample code on Occlum
```
cd /root/occlum/demos/tensorflow/tensorflow_training
bash ./run_tensorflow_on_occlum.sh
```
