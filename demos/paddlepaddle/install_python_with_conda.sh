#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

# 1. Init occlum workspace
[ -d occlum_instance ] || occlum new occlum_instance

# 2. Install python and dependencies to specified position
[ -f Miniconda3-latest-Linux-x86_64.sh ] || wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
[ -d miniconda ] || bash ./Miniconda3-latest-Linux-x86_64.sh -b -p $script_dir/miniconda
$script_dir/miniconda/bin/conda create --prefix $script_dir/python-occlum -y matplotlib numpy python=3.8.10 paddlepaddle==2.4.2 -c paddle

CORE_PY=$script_dir/python-occlum/lib/python3.8/site-packages/paddle/fluid/core.py
IMAGE_PY=$script_dir/python-occlum/lib/python3.8/site-packages/paddle/dataset/image.py

# Adjust the source code to run in Occlum
sed -i "186 i \    elif sysstr == 'occlum':\n        return True" $CORE_PY
sed -ie "37,64d" $IMAGE_PY
sed -i "37 i \try:\n    import cv2\nexcept ImportError:\n     cv2 = None" $IMAGE_PY

# Download the dataset
DATASET=$script_dir/mnist

[ -d $DATASET ] && exit 0

TRAIN_IMAGE=train-images-idx3-ubyte.gz
TRAIN_LABEL=train-labels-idx1-ubyte.gz
TEST_IMAGE=t10k-images-idx3-ubyte.gz
TEST_LABEL=t10k-labels-idx1-ubyte.gz
URL=http://yann.lecun.com/exdb/mnist

mkdir $DATASET
wget $URL/$TRAIN_IMAGE -P $DATASET
wget $URL/$TRAIN_LABEL -P $DATASET
wget $URL/$TEST_IMAGE  -P $DATASET
wget $URL/$TEST_LABEL  -P $DATASET
