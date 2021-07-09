#!/bin/bash
set -e

export LD_LIBRARY_PATH="/usr/local/lib:$LD_LIBRARY_PATH"

cd grpc_src/examples/cpp/helloworld
mkdir -p cmake/build && cd cmake/build
cmake ../.. \
	-DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS="-fPIC -pie" -DCMAKE_C_FLAGS="-fPIC -pie"
make -j$(nproc)
