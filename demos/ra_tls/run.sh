#!/bin/bash
set -ex

postfix=$1

if [ "$postfix" != "server" ] && [ "$postfix" != "client" ]; then
    echo "input error args, it should be:" 
    echo "./run.sh server"
    echo "./run.sh client"
    exit 1
fi

pushd occlum_$postfix
occlum run /bin/$postfix
popd
