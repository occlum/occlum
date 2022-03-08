#!/bin/bash

apt-get update
apt-get install -y openjdk-11-jre
apt-get install -y netcat

#The openjdk has a broken symlink, remove it as a workaround
rm /usr/lib/jvm/java-11-openjdk-amd64/lib/security/blacklisted.certs

# Redis
[ -f redis-${REDIS_VERSION}.tar.gz ] || wget http://download.redis.io/releases/redis-${REDIS_VERSION}.tar.gz
[ -d redis-${REDIS_VERSION} ] || tar -zxvf redis-${REDIS_VERSION}.tar.gz
cd redis-${REDIS_VERSION}
make
cd ../

# Flink
[ -f flink-${FLINK_VERSION}-bin-scala_2.11.tgz ] || wget https://archive.apache.org/dist/flink/flink-${FLINK_VERSION}/flink-${FLINK_VERSION}-bin-scala_2.11.tgz
[ -d flink-${FLINK_VERSION}-bin-scala_2.11 ] || tar -zxvf flink-${FLINK_VERSION}-bin-scala_2.11.tgz

# Analytics Zoo
[ -f analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-serving.jar ] || wget https://repo1.maven.org/maven2/com/intel/analytics/zoo/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION/$ANALYTICS_ZOO_VERSION/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-serving.jar
[ -f analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-http.jar ] || wget https://repo1.maven.org/maven2/com/intel/analytics/zoo/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION/$ANALYTICS_ZOO_VERSION/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-http.jar

# models
[ -d resnet50 ] || mkdir resnet50 && \
cd resnet50
[ -f resnet_v1_50.bin ] || wget -c "https://sourceforge.net/projects/analytics-zoo/files/analytics-zoo-models/openvino/2018_R5/resnet_v1_50.bin/download" -O resnet_v1_50.bin && \
[ -f resnet_v1_50.xml ] || wget -c "https://sourceforge.net/projects/analytics-zoo/files/analytics-zoo-models/openvino/2018_R5/resnet_v1_50.xml/download" -O resnet_v1_50.xml
