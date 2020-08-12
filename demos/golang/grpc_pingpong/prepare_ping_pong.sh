#!/bin/bash -x
set -e

BLUE='\033[1;34m'
NC='\033[0m'

# sanity check
CURR_FILE=go.mod
if [ -f "$CURR_FILE" ]; then
        rm $CURR_FILE
fi
CURR_FILE=go.sum
if [ -f "$CURR_FILE" ]; then
        rm $CURR_FILE
fi
CURR_FILE=ping
if [ -f "$CURR_FILE" ]; then
        rm $CURR_FILE
fi
CURR_FILE=pong
if [ -f "$CURR_FILE" ]; then
        rm $CURR_FILE
fi
CURR_FILE=pingpong/pingpong_grpc.pb.go
if [ -f "$CURR_FILE" ]; then
        rm $CURR_FILE
fi
CURR_FILE=pingpong/pingpong.pb.go
if [ -f "$CURR_FILE" ]; then
        rm $CURR_FILE
fi

# assume that protoc is installed
CURR_FILE=$(which protoc)
if [ ! -f "$CURR_FILE" ]; then
        echo "Please install protoc"
        exit 1
fi

# install protoc-gen-go and protoc-gen-go-grpc plugin
CURR_FILE=$(which protoc-gen-go)
if [ ! -f "$CURR_FILE" ]; then
        go install google.golang.org/protobuf/cmd/protoc-gen-go
fi
CURR_FILE=$(which protoc-gen-go-grpc)
if [ ! -f "$CURR_FILE" ]; then
        go install google.golang.org/grpc/cmd/protoc-gen-go-grpc
fi

# compiling pingpong gRPC .proto file
protoc --go-grpc_out=. --go_out=. pingpong/pingpong.proto

# enable and initialize Go modules for package management 
export GO111MODULE=on
go mod init grpc_pingpong

# build pong image
go build pong.go

# build ping image
go build ping.go

# prepare occlum images
occlum-go build -o occlum_pong -buildmode=pie pong.go
occlum-go build -o occlum_ping -buildmode=pie ping.go
