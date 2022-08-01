#!/bin/bash
set -x

REDIS_HOST=127.0.0.1
REDIS_PORT=6379
FLINK_TASK_MANAGER_IP=127.0.0.1
FLINK_JOB_MANAGER_REST_PORT=8081
FLINK_TASK_MANAGER_DATA_PORT=6124

./start-redis.sh &
echo "redis started"

./start-flink-jobmanager.sh &
echo "flink-jobmanager started"

./init-occlum-taskmanager.sh
echo "occlum flink taskmanager image built"
while ! nc -z $FLINK_TASK_MANAGER_IP $FLINK_JOB_MANAGER_REST_PORT; do
  sleep 1
done
./start-flink-taskmanager.sh &
echo "flink-taskmanager started"

while ! nc -z $REDIS_HOST $REDIS_PORT; do
  sleep 1
done
./start-http-frontend.sh &
echo "http-frontend started"

while ! nc -z $FLINK_TASK_MANAGER_IP $FLINK_TASK_MANAGER_DATA_PORT; do
  sleep 1
done
./start-cluster-serving-job.sh &
echo "cluster-serving-job started"

