#!/bin/bash

set -x

echo "### Launching HTTP Frontend ###"

redis_host=127.0.0.1
core_num=$CORE_NUM

java \
    -Xms2g \
    -Xmx8g \
    -XX:ActiveProcessorCount=${core_num} \
    -Dcom.intel.analytics.zoo.shaded.io.netty.tryReflectionSetAccessible=true \
    -Dakka.http.host-connection-pool.max-connections=100 \
    -Dakka.http.host-connection-pool.max-open-requests=128 \
    -Dakka.actor.default-dispatcher.fork-join-executor.parallelism-min=100 \
    -Dakka.actor.default-dispatcher.fork-join-executor.parallelism-max=120 \
    -Dakka.actor.default-dispatcher.fork-join-executor.parallelism-factor=1 \
    -jar analytics-zoo-bigdl_${BIGDL_VERSION}-spark_${SPARK_VERSION}-${ANALYTICS_ZOO_VERSION}-http.jar \
    --redisHost "${redis_host}" \
    --tokensPerSecond 30 \
    --tokenBucketEnabled true \
    --parallelism ${core_num} | tee ./http-frontend-sgx.log
