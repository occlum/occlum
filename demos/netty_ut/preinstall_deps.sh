#!/bin/bash
set -e

OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y openjdk-11-jdk
  rm -rf /usr/lib/jvm/java-11-openjdk-amd64/lib/security/blacklisted.certs
else
  echo "Unsupported OS: $OS"
  exit 1
fi

echo "Install dependencies success"
