#!/bin/bash
# set -x

echo "### Launching Redis ###"
REDIS_PORT=6379

$REDIS_HOME/src/redis-server --port $REDIS_PORT \
    --protected-mode no --maxmemory 6g | tee ./redis-sgx.log
