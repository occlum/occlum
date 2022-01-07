#!/bin/bash
set -x

REDIS_HOST=127.0.0.1
REDIS_PORT=6379
FLINK_TASK_MANAGER_IP=127.0.0.1
FLINK_JOB_MANAGER_REST_PORT=8081
FLINK_TASK_MANAGER_DATA_PORT=6124
MAX_LOOP_NUM=60

./start-redis.sh &
echo "redis started"

./start-flink-jobmanager.sh &
echo "flink-jobmanager started"

./init-occlum-taskmanager.sh
echo "occlum flink taskmanager image built"
index=0
while ! nc -z $FLINK_TASK_MANAGER_IP $FLINK_JOB_MANAGER_REST_PORT; do
  sleep 10

  #wait 10 minutes
  index=$(($index+1))
  if [[ $index -eq MAX_LOOP_NUM ]]; then
	exit -1
  fi
done
./start-flink-taskmanager.sh &
echo "flink-taskmanager started"

index=0
while ! nc -z $REDIS_HOST $REDIS_PORT; do
  sleep 10

  #wait 10 minutes
  index=$(($index+1))
  if [[ $index -eq MAX_LOOP_NUM ]]; then
	exit -1
  fi
done
./start-http-frontend.sh &
echo "http-frontend started"

index=0
while ! nc -z $FLINK_TASK_MANAGER_IP $FLINK_TASK_MANAGER_DATA_PORT; do
  sleep 10
  
  #wait 10 minutes
  index=$(($index+1))
  if [[ $index -eq MAX_LOOP_NUM ]]; then
	exit -1
  fi
done
./start-cluster-serving-job.sh &
echo "cluster-serving-job started"

