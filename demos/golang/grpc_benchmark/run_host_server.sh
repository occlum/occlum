#!/bin/bash

export GOPATH=$PWD
out_dir=$PWD/bin
port=50051
# Launch the server in background
${out_dir}/server --port=${port} --test_name="Server_gRPC"&
server_pid=$(echo $!)
echo "server_pid = $server_pid"
