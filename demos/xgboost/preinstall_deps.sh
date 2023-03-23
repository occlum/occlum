#!/bin/bash
set -e

# Tell CMake to search for packages in Occlum toolchain's directory only
export PKG_CONFIG_LIBDIR=/usr/local/occlum/x86_64-linux-musl/lib

# Install dependencies
OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y python3-pip python3-setuptools
else
  yum install -y python3-pip python3-setuptools
fi
pip3 install kubernetes

echo "Install dependencies success"

# Install CMake
./install_cmake.sh
