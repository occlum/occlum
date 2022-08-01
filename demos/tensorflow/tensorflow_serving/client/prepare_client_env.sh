#!/bin/bash
set -e

OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y libgl1-mesa-glx
else
  yum install -y mesa-libGL
fi

pip3 install -r requirements.txt -v
