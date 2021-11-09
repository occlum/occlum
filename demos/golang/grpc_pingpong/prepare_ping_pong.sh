#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

# sanity check
FILE_SET="
occlum_ping
occlum_pong
go.sum"
for CURR_FILE in $FILE_SET
do
        if [ -f "$CURR_FILE" ]; then
                rm $CURR_FILE
        fi
done

DIR_SET="
github.com
occlum_ping_instance
occlum_pong_instance"
for CURR_DIR in $DIR_SET
do
        if [ -d "$CURR_DIR" ]; then
                rm -fr $CURR_DIR
        fi
done

# enable Go modules for package management 
export GO111MODULE=on

# update PATH so that the protoc compiler can find the plugin:
export PATH="$PATH:$(go env GOPATH)/bin"

# assume that protoc is installed
if ! type "protoc" > /dev/null 2>&1; then
        echo "Please install protoc"
        exit 1
fi

# install protoc-gen-go and protoc-gen-go-grpc plugin
if ! type "protoc-gen-go" > /dev/null 2>&1; then
        occlum-go get google.golang.org/protobuf/cmd/protoc-gen-go
fi
if ! type "protoc-gen-go-grpc" > /dev/null 2>&1; then
        occlum-go get google.golang.org/grpc/cmd/protoc-gen-go-grpc
fi

# compiling pingpong gRPC .proto file
export PATH=$PATH:$HOME/go/bin
export GOPRIVATE=github.com/occlum/demos/\*
protoc --proto_path=pingpong --go-grpc_out=. --go_out=. pingpong/pingpong.proto

# prepare occlum images
occlum-go mod download golang.org/x/net
occlum-go mod download google.golang.org/grpc
occlum-go mod download github.com/golang/protobuf
occlum-go mod download golang.org/x/sys
occlum-go mod download golang.org/x/text
occlum-go mod download google.golang.org/genproto
occlum-go mod download google.golang.org/protobuf

occlum-go build -o occlum_pong pong.go
occlum-go build -o occlum_ping ping.go
