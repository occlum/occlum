#!/bin/bash
set -e

# Download netty testsuite, junit platform and related dependencies

if [ ! -d "netty" ]; then
    wget -i ./ut-jar.url -P ./netty
fi

echo "Build Netty unit test success"
