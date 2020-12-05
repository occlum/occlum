#!/bin/bash
make clean
make TESTS="hello_world" TEST_DEPS="" BENCHES=""
export OCCLUM_LOG_LEVEL=debug
export RUST_BACKTRACE=1
cd ../build/test/ && occlum run /bin/hello_world
