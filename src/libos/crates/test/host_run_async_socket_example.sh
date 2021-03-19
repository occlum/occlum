#!/bin/bash
source_dir=${PWD}

cd ${source_dir}/../async-socket/
cargo b --examples --release
cd ${source_dir}/../target/release/examples/
./tcp_echo &

sleep 2

cd ${source_dir}/tcp_client
cargo run --release

sleep 2
# kill server and clients
for pid in $(/bin/ps | grep "tcp_client" | awk '{print $1}'); do kill -9 $pid; done
for pid in $(/bin/ps | grep "tcp_echo" | awk '{print $1}'); do kill -9 $pid; done