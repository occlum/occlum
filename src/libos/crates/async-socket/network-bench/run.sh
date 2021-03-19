#!/bin/sh
bench_dir=$PWD

benchmark(){
    # kill server and clients
    for pid in $(/bin/ps | grep "client" | awk '{print $1}'); do kill -9 $pid; done
    for pid in $(/bin/ps | grep "server" | awk '{print $1}'); do kill -9 $pid; done
    for pid in $(/bin/ps | grep "app" | awk '{print $1}'); do kill -9 $pid; done
    for pid in $(/bin/ps | grep "tcp_echo" | awk '{print $1}'); do kill -9 $pid; done

    cd $bench_dir
    ./server $port &
    sleep 2
    ./client 127.0.0.1 $port $block_size $client_num $req_num | tee -a "$file_name"

    # kill server and clients
    for pid in $(/bin/ps | grep "client" | awk '{print $1}'); do kill -9 $pid; done
    for pid in $(/bin/ps | grep "server" | awk '{print $1}'); do kill -9 $pid; done

    sleep 2

    cd $bench_dir/../../target/release/examples/
    ./tcp_echo $port &
    sleep 2
    cd $bench_dir
    ./client 127.0.0.1 $port $block_size $client_num $req_num | tee -a "$file_name"

    # kill server and clients
    for pid in $(/bin/ps | grep "client" | awk '{print $1}'); do kill -9 $pid; done
    for pid in $(/bin/ps | grep "tcp_echo" | awk '{print $1}'); do kill -9 $pid; done

    sleep 2

    cd $bench_dir/../examples/sgx/bin
    ./app $port &
    sleep 2
    cd $bench_dir
    ./client 127.0.0.1 $port $block_size $client_num $req_num  | tee -a "$file_name"

    # kill server and clients
    for pid in $(/bin/ps | grep "client" | awk '{print $1}'); do kill -9 $pid; done
    for pid in $(/bin/ps | grep "app" | awk '{print $1}'); do kill -9 $pid; done
}

gcc -O2 -pthread server.c -o server
gcc -O2 -pthread client.c -o client

cd $bench_dir/..
cargo b --release --examples

cd $bench_dir/../examples/sgx
make

file_name="benchmark_result.txt"

port=3456
block_size=32768

req_num=100000
for client_num in 1 10 20 30 40 50 60 70 80 90 100
do
   benchmark
   sleep 5
done