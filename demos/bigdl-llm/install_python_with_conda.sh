#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

# Install python and dependencies to specified position
[ -f Miniconda3-latest-Linux-x86_64.sh ] || wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
[ -d miniconda ] || bash ./Miniconda3-latest-Linux-x86_64.sh -b -p $script_dir/miniconda
$script_dir/miniconda/bin/conda create \
    --prefix $script_dir/python-occlum -y \
    python=3.9.11

# Install BigDL LLM
$script_dir/python-occlum/bin/pip install torch==2.1.0 --index-url https://download.pytorch.org/whl/cpu
$script_dir/python-occlum/bin/pip install --pre --upgrade bigdl-llm[all] bigdl-llm[serving]
