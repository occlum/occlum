#!/bin/bash
benchmark=redis_benchmark
occlum_dir=/usr/local/grpc/
occlum_glibc=/opt/occlum/glibc/lib/
set -ex

postfix=$1

if [ "$postfix" != "server" ] && [ "$postfix" != "client" ]; then
    echo "input error args, it should be:" 
    echo "./run.sh server"
    echo "./run.sh client"
    exit 1
fi

pushd occlum_instance_$postfix
occlum run /bin/$postfix
popd
