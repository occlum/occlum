#!/bin/bash
set -e

total=200
concurrency=50

show_usage() {
    echo ""
    cat <<EOF
Usage:
    run_stress_test.sh [-n <total_requests>] [-c <concurrent_workers>]

    The combination of -c and -n are critical in how the benchmarking is done.
    It takes the -c argument and spawns that many worker goroutines. In parallel
    these goroutines each do their share (n / c) requests.
    For example with the default -c 50 -n 200 options we would spawn 50 goroutines
    which in parallel each do 4 requests.

EOF
}

exit_error() {
    echo "Error: $@" >&2
    show_usage
    exit 1
}

while [ -n "$1" ]; do
    case "$1" in
    -n) [ -n "$2" ] && total=$2 ; shift 2 || exit_error "empty total number of requests to run"             ;;
    -c) [ -n "$2" ] && concurrency=$2 ; shift 2 || exit_error "empty number of workers to run concurrently" ;;
    *) exit_error "Unknown option: $1"                                                                      ;;
    esac
done

# Use ghz tool to run the stress test
./ghz_src/dist/ghz \
    --insecure -n $total -c $concurrency \
    --proto ./grpc_src/examples/protos/helloworld.proto \
    --call helloworld.Greeter.SayHello \
    -d '{"name":"World"}' localhost:50051
