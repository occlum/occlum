#!/bin/bash
set -e

rm -rf flink-1.15.2*
wget https://archive.apache.org/dist/flink/flink-1.15.2/flink-1.15.2-bin-scala_2.12.tgz
tar -xvzf flink-1.15.2-bin-scala_2.12.tgz

echo "Download Flink Success"
