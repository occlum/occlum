#!/bin/bash
sudo apt-get update
sudo apt-get install openjdk-11-jdk
rm -rf /usr/lib/jvm/java-11-openjdk-amd64/lib/security/blacklisted.certs

# Download netty testsuite, junit platform and related dependencies

if [ ! -d "netty" ]; then
    wget -i ./ut-jar.url -P ./netty
fi
