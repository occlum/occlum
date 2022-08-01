#!/bin/bash
set -e

OS=`awk -F= '/^NAME/{print $2}' /etc/os-release`
if [ "$OS" == "\"Ubuntu\"" ]; then
  apt-get update -y && apt-get install -y openjdk-11-jre netcat
  # The openjdk has a broken symlink, remove it as a workaround
  rm -f /usr/lib/jvm/java-11-openjdk-amd64/lib/security/blacklisted.certs
else
  echo "Unsupported OS: $OS"
  exit 1
fi

# Redis
rm -rf redis-${REDIS_VERSION}*
wget http://download.redis.io/releases/redis-${REDIS_VERSION}.tar.gz && \
tar -zxvf redis-${REDIS_VERSION}.tar.gz
rm redis-${REDIS_VERSION}.tar.gz
cd redis-${REDIS_VERSION}
make
cd ../

# Flink
rm -rf flink-${FLINK_VERSION}*
wget https://archive.apache.org/dist/flink/flink-${FLINK_VERSION}/flink-${FLINK_VERSION}-bin-scala_2.11.tgz
tar -zxvf flink-${FLINK_VERSION}-bin-scala_2.11.tgz
rm flink-${FLINK_VERSION}-bin-scala_2.11.tgz

# Analytics Zoo
rm -rf analytics-zoo-bigdl_*
wget https://repo1.maven.org/maven2/com/intel/analytics/zoo/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION/$ANALYTICS_ZOO_VERSION/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-serving.jar
wget https://repo1.maven.org/maven2/com/intel/analytics/zoo/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION/$ANALYTICS_ZOO_VERSION/analytics-zoo-bigdl_$BIGDL_VERSION-spark_$SPARK_VERSION-$ANALYTICS_ZOO_VERSION-http.jar

# models
rm -rf resnet50 && mkdir resnet50 && \
cd resnet50 && \
wget -c "https://sourceforge.net/projects/analytics-zoo/files/analytics-zoo-models/openvino/2018_R5/resnet_v1_50.bin/download" -O resnet_v1_50.bin && \
wget -c "https://sourceforge.net/projects/analytics-zoo/files/analytics-zoo-models/openvino/2018_R5/resnet_v1_50.xml/download" -O resnet_v1_50.xml
