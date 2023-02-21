#!/bin/bash
set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

# 1. Init occlum workspace
[ -d occlum_instance ] || occlum new occlum_instance

# 2. Install python and dependencies to specified position
[ -f Miniconda3-latest-Linux-x86_64.sh ] || wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
[ -d miniconda ] || bash ./Miniconda3-latest-Linux-x86_64.sh -b -p $script_dir/miniconda
$script_dir/miniconda/bin/conda create --prefix $script_dir/python-occlum -y python=3.9.11 flask=2.2.2 flask-restful=0.3.9 jinja2=3.1.2  werkzeug

