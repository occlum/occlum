#!/bin/bash
set -e

postfix=$1
request=$2
file=${3:-/host/secret}

GRPC_ADDR="localhost:50051"

if [ "$postfix" == "server" ]; then
    pushd occlum_server
    occlum run /bin/server ${GRPC_ADDR}
    popd
elif [ "$postfix" == "client" ]; then
    pushd occlum_client
    occlum run /bin/client ${GRPC_ADDR} ${request} ${file}
    popd
else
    echo "input error args, it should be:" 
    echo "./run.sh server"
    echo "./run.sh client request_secret"
    exit 1
fi

