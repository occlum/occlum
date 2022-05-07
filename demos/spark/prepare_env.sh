#!/bin/bash

# Env variables
container=intelanalytics/bigdl-ppml-trusted-big-data-ml-scala-occlum
tag=2.1.0-SNAPSHOT
SPARK_HOME=/opt/spark

# Install openjdk11
apt-get update
apt-get install -y openjdk-11-jre
#The openjdk has a broken symlink (blacklisted.certs), remove it as a workaround
rm /usr/lib/jvm/java-11-openjdk-amd64/lib/security/blacklisted.certs

# Copy modified spark from analytics zoo docker image
docker pull ${container}:${tag}
docker create --name az_spark ${container}:${tag}
docker cp az_spark:${SPARK_HOME} ${SPARK_HOME}
docker rm az_spark
