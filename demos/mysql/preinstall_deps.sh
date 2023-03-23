#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Start installing dependencies.${NC}"

DEPS="libnuma-dev libboost-all-dev"

OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y ${DEPS}
  # Install sysbench for benchmarking purpose
  apt-get install -y sysbench
else
  echo "Unsupported OS: $OS"
  exit 1
fi

BOOST="boost_1_77_0"
wget https://boostorg.jfrog.io/artifactory/main/release/1.77.0/source/${BOOST}.tar.bz2
tar --bzip2 -xf ${BOOST}.tar.bz2
pushd ${BOOST}
./bootstrap.sh --prefix=/usr --with-python=python3 &&
./b2 stage -j4 threading=multi link=shared
./b2 install threading=multi link=shared
popd

echo -e "${BLUE}Finish installing dependencies.${NC}"
