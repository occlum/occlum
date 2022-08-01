#!/bin/bash
set -e

OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y openjdk-11-jre
  # The openjdk has a broken symlink, remove it as a workaround
  rm -f /usr/lib/jvm/java-11-openjdk-amd64/lib/security/blacklisted.certs
else
  echo "Unsupported OS: $OS"
  exit 1
fi

rm -rf flink-1.10.1*
wget https://archive.apache.org/dist/flink/flink-1.10.1/flink-1.10.1-bin-scala_2.11.tgz
tar -xvzf flink-1.10.1-bin-scala_2.11.tgz

echo "Download Flink Success"
