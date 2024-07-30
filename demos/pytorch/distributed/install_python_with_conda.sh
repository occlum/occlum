#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

# Install python and dependencies to specified position
[ -f Miniconda3-latest-Linux-x86_64.sh ] || wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
[ -d miniconda ] || bash ./Miniconda3-latest-Linux-x86_64.sh -b -p $script_dir/miniconda
$script_dir/miniconda/bin/conda create --prefix $script_dir/python-occlum -y \
    python=3.8.10 numpy=1.22.3 scipy=1.7.3 scikit-learn=1.0 pandas=1.3 \
    Cython pytorch torchvision -c pytorch

# Remove miniconda and installation scripts
rm -rf ./Miniconda3-latest-Linux-x86_64.sh $script_dir/miniconda