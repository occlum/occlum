#!/bin/bash
set -e

OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y python3-pip
else
  yum install -y python3-pip
fi

pip3 install --upgrade pip
pip3 install --upgrade tensorflow==2.4 protobuf==3.19.2
./download_model.sh
python3 ./model_graph_to_saved_model.py --import_path ./models/resnet50-v15-fp32/resnet50-v15-fp32.pb --export_dir ./resnet50-v15-fp32 --model_version 1 --inputs input --outputs predict
