#
# Copyright (c) 2022 Intel Corporation
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -ex

export ABSEIL_PATH=${GRPC_PATH}/third_party/abseil-cpp

if [ ! -d "${BUILD_TYPE}" ]; then
    BUILD_TYPE=Release
fi

# build and install abseil library
# https://abseil.io/docs/cpp/quickstart-cmake.html
if [ ! -d "${ABSEIL_PATH}/build" ]; then
    mkdir -p ${ABSEIL_PATH}/build
    cd ${ABSEIL_PATH}/build
    cmake -DCMAKE_CXX_STANDARD=11 -DCMAKE_POSITION_INDEPENDENT_CODE=TRUE \
          -DCMAKE_BUILD_TYPE=${BUILD_TYPE} -DCMAKE_INSTALL_PREFIX=${INSTALL_PREFIX} ..
    make -j `nproc`
    make install
    cd -
fi

# build and install grpc library
mkdir -p ${GRPC_PATH}/build
cd ${GRPC_PATH}/build
cmake -DgRPC_INSTALL=ON -DgRPC_ABSL_PROVIDER=package -DgRPC_BUILD_TESTS=OFF \
      -DgRPC_BUILD_CSHARP_EXT=OFF -DgRPC_BUILD_GRPC_CSHARP_PLUGIN=OFF \
      -DgRPC_BUILD_GRPC_PHP_PLUGIN=OFF -DgRPC_BUILD_GRPC_RUBY_PLUGIN=OFF \
      -DDEFINE_SGX_RA_TLS_OCCLUM_BACKEND=ON \
      -DCMAKE_BUILD_TYPE=${BUILD_TYPE} -DCMAKE_INSTALL_PREFIX=${INSTALL_PREFIX} ..
make -j `nproc`
make install
cd -
