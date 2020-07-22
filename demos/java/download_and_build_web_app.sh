#!/bin/bash
set -e

# 1. Download the demo
rm -rf gs-messaging-stomp-websocket && mkdir gs-messaging-stomp-websocket
cd gs-messaging-stomp-websocket
git clone https://github.com/spring-guides/gs-messaging-stomp-websocket.git .
git checkout -b 2.1.6.RELEASE tags/2.1.6.RELEASE

# 2. Build the Fat JAR file with Maven
cd complete
export LD_LIBRARY_PATH=/opt/occlum/toolchains/gcc/x86_64-linux-musl/lib
export JAVA_HOME=/opt/occlum/toolchains/jvm/java-11-alibaba-dragonwell
./mvnw clean package
