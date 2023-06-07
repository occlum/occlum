#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
python_dir="$script_dir/python-occlum"

[ -d occlum_instance ] || occlum new occlum_instance

if [ ! -d $python_dir ];then
    echo "Error: cannot stat '$python_dir' directory"
    exit 1
fi

cd occlum_instance
# Copy files into Occlum Workspace and build
if [ ! -L "image/bin/python3" ];then
    rm -rf image
    copy_bom -f ../tensorflow_training.yaml --root image --include-dir /opt/occlum/etc/template
    yq '.resource_limits.user_space_size.init = "5400MB" |
        .resource_limits.kernel_space_heap_size.init = "1024MB" |
        .env.default += ["PYTHONHOME=/opt/python-occlum", "OMP_NUM_THREADS=1"]' \
        -i Occlum.yaml

    occlum build
fi

# Run the tensorflow demo
echo -e "${BLUE}occlum run /bin/python3 demo.py${NC}"
occlum run /bin/python3 /bin/demo.py
