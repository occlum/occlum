#!/bin/bash
TEST=spawn
make clean
make TESTS="$TEST getpid" TEST_DEPS="" BENCHES=""
export OCCLUM_LOG_LEVEL=debug
export RUST_BACKTRACE=1
cd ../build/test/ && occlum run /bin/$TEST
