#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
echo -e "${BLUE}Start installing dependencies.${NC}"

DEPS="libgflags-dev libsnappy-dev zlib1g-dev libbz2-dev liblz4-dev libzstd-dev"

OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y ${DEPS}
else
  echo "Unsupported OS: $OS"
  exit 1
fi

echo -e "${BLUE}Finish installing dependencies.${NC}"
