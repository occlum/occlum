#!/bin/bash
set -e

postfix=$1
request=$2
file=${3:-/host/secret}

if [ "$postfix" != "server" ] && [ "$postfix" != "client" ]; then
    echo "input error args, it should be:" 
    echo "./run.sh server"
    echo "./run.sh client"
    exit 1
fi

pushd occlum_$postfix
occlum run /bin/$postfix ${request} ${file}
popd
