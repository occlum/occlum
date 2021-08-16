#!/bin/bash

apt-get update
apt-get install -y openjdk-11-jdk
apt-get install -y netcat

# Redis
wget http://download.redis.io/releases/redis-${REDIS_VERSION}.tar.gz && \
tar -zxvf redis-${REDIS_VERSION}.tar.gz
rm redis-${REDIS_VERSION}.tar.gz
cd redis-${REDIS_VERSION}
make
cd ../

# Flink
wget https://archive.apache.org/dist/flink/flink-${FLINK_VERSION}/flink-${FLINK_VERSION}-bin-scala_2.11.tgz
tar -zxvf flink-${FLINK_VERSION}-bin-scala_2.11.tgz
rm flink-${FLINK_VERSION}-bin-scala_2.11.tgz

# Analytics Zoo
wget https://repo1.maven.org/maven2/com/intel/analytics/zoo/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION/$ANALYTICS_ZOO_VERSION/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-serving.jar
wget https://repo1.maven.org/maven2/com/intel/analytics/zoo/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION/$ANALYTICS_ZOO_VERSION/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-http.jar

# models
mkdir resnet50 && \
cd resnet50 && \
wget -c "https://sourceforge.net/projects/analytics-zoo/files/analytics-zoo-models/openvino/2018_R5/resnet_v1_50.bin/download" -O resnet_v1_50.bin && \
wget -c "https://sourceforge.net/projects/analytics-zoo/files/analytics-zoo-models/openvino/2018_R5/resnet_v1_50.xml/download" -O resnet_v1_50.xml
