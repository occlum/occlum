#!/bin/bash
#set -x

# Stop cluster serving
${FLINK_HOME}/bin/flink list | grep RUNNING | awk '{print $4}' | xargs ${FLINK_HOME}/bin/flink cancel
ps -ef | grep http.jar | grep -v grep | awk '{print $2}' | xargs kill -9

# Stop Flink
ps -ef | grep -e TaskManagerRunner -e StandaloneSessionClusterEntrypoint | grep -v grep | awk '{print $2}' | xargs kill -9

# Stop Redis
${REDIS_HOME}/src/redis-cli shutdown
