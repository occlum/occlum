#!/bin/bash
source_dir=${PWD}

cd ${source_dir}/../third_parties/io-uring/
cargo b --examples --release
cargo run --example tcp_echo --release &

sleep 2

cd ${source_dir}/tcp_client
cargo run --release

sleep 2
# kill server and clients
for pid in $(/bin/ps | grep "tcp_client" | awk '{print $1}'); do kill -9 $pid; done
for pid in $(/bin/ps | grep "tcp_echo" | awk '{print $1}'); do kill -9 $pid; done