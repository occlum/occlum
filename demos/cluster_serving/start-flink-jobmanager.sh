#!/bin/bash

set -x

core_num=$CORE_NUM
job_manager_host=127.0.0.1
job_manager_rest_port=8081
job_manager_rpc_port=6123
flink_home=$FLINK_HOME
flink_version=$FLINK_VERSION

echo "### Launching Flink Jobmanager ###"

jars=(${flink_home}/lib/*.jar)
jars_cp=$( IFS=$':'; echo "${jars[*]}" )

java \
    -Xms5g \
    -Xmx10g \
    -XX:ActiveProcessorCount=${core_num} \
    -Dorg.apache.flink.shaded.netty4.io.netty.tryReflectionSetAccessible=true \
    -Dorg.apache.flink.shaded.netty4.io.netty.eventLoopThreads=${core_num} \
    -Dcom.intel.analytics.zoo.shaded.io.netty.tryReflectionSetAccessible=true \
    -Dlog.file=${flink_home}/log/flink-sgx-standalonesession-1-sgx-ICX-LCC.log \
    -Dlog4j.configuration=file:${flink_home}/conf/log4j.properties \
    -Dlogback.configurationFile=file:${flink_home}/conf/logback.xml \
    -classpath ${jars_cp} org.apache.flink.runtime.entrypoint.StandaloneSessionClusterEntrypoint \
    --configDir ${flink_home}/conf \
    -D rest.bind-address=${job_manager_host} \
    -D rest.bind-port=${job_manager_rest_port} \
    -D jobmanager.rpc.address=${job_manager_host} \
    -D jobmanager.rpc.port=${job_manager_rpc_port} \
    -D jobmanager.heap.size=5g \
    --executionMode cluster | tee ./flink-jobmanager-sgx.log

