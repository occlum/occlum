#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

# 1. Init occlum workspace
[ -d occlum_instance ] || mkdir occlum_instance
cd occlum_instance
[ -d image ] || occlum init

# 2. Install python and dependencies to specified position
cd ../
wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
bash ./Miniconda3-latest-Linux-x86_64.sh -b -p $script_dir/miniconda
$script_dir/miniconda/bin/conda create --prefix $script_dir/occlum_instance/image/opt/python-occlum -y python=3.7 numpy pandas scipy=1.3.1 Cython scikit-learn=0.21.1

