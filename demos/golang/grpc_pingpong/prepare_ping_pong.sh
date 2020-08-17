#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

# sanity check
FILE_SET="go.mod
ping
pong
occlum_ping
occlum_pong
go.mod
go.sum
pingpong/pingpong.pb.go
pingpong/pingpong_grpc.pb.go"
for CURR_FILE in $FILE_SET
do
        if [ -f "$CURR_FILE" ]; then
                rm $CURR_FILE
        fi
done

DIR_SET="occlum_ping_instance
occlum_pong_instance"
for CURR_DIR in $DIR_SET
do
        if [ -d "$CURR_DIR" ]; then
                rm -fr $CURR_DIR
        fi
done

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
