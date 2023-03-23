#!/bin/bash
set -e

rm -rf flink-1.10.1*
wget https://archive.apache.org/dist/flink/flink-1.10.1/flink-1.10.1-bin-scala_2.11.tgz
tar -xvzf flink-1.10.1-bin-scala_2.11.tgz

echo "Download Flink Success"
