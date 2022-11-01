#!/bin/bash
set -e

export GOPATH=$HOME/go
out_dir=$PWD/bin
rm -rf ${out_dir}
mkdir ${out_dir}
rm -f go.mod
occlum-go mod init grpc_benchmark
occlum-go mod tidy
occlum-go get -u google.golang.org/grpc@v1.50.1
cd ${GOPATH}/pkg/mod/google.golang.org/grpc@v1.50.1
occlum-go build -o ${out_dir}/server ./benchmark/server/main.go
occlum-go build -o ${out_dir}/client ./benchmark/client/main.go
