#!/bin/bash
set -e

export GOPATH=$PWD
out_dir=$PWD/bin
occlum-go get -u google.golang.org/grpc
cd src/google.golang.org/grpc/
rm -rf ${out_dir}
mkdir ${out_dir}
occlum-go build -o ${out_dir}/server $GOPATH/src/google.golang.org/grpc/benchmark/server/main.go && occlum-go build -o ${out_dir}/client $GOPATH/src/google.golang.org/grpc/benchmark/client/main.go
