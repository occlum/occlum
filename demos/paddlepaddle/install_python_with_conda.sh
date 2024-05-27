#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

# 1. Init occlum workspace
[ -d occlum_instance ] || occlum new occlum_instance

# 2. Install python and dependencies to specified position
[ -f Miniconda3-latest-Linux-x86_64.sh ] || wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
[ -d miniconda ] || bash ./Miniconda3-latest-Linux-x86_64.sh -b -p $script_dir/miniconda
$script_dir/miniconda/bin/conda create --prefix $script_dir/python-occlum -y matplotlib numpy python=3.8.10 paddlepaddle==2.4.2 -c paddle

# Remove miniconda and installation scripts
rm -rf ./Miniconda3-latest-Linux-x86_64.sh $script_dir/miniconda

CORE_PY=$script_dir/python-occlum/lib/python3.8/site-packages/paddle/fluid/core.py
IMAGE_PY=$script_dir/python-occlum/lib/python3.8/site-packages/paddle/dataset/image.py

# Adjust the source code to run in Occlum
sed -i "186 i \    elif sysstr == 'occlum':\n        return True" $CORE_PY
sed -ie "37,64d" $IMAGE_PY
sed -i "37 i \try:\n    import cv2\nexcept ImportError:\n     cv2 = None" $IMAGE_PY


# Download the dataset
git clone https://github.com/fgnt/mnist.git

