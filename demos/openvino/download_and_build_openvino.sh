#!/bin/bash
PREFIX=/usr/local/occlum/x86_64-linux-musl
THREADING=TBB
set -e

show_usage() {
    echo
    echo "Usage: $0 [--threading <TBB/OMP>]"
    echo
    exit 1
}

build_opencv() {
    rm -rf deps/zlib && mkdir -p deps/zlib
    pushd deps/zlib
    git clone https://github.com/madler/zlib .
    CC=occlum-gcc CXX=occlum-g++ ./configure --prefix=$PREFIX
    make
    sudo make install
    popd

    rm -rf deps/opencv && mkdir -p deps/opencv
    pushd deps/opencv
    git clone https://github.com/opencv/opencv .
    git checkout tags/4.1.0 -b 4.1.0
    mkdir build
    cd build
    cmake ../ \
      -DCMAKE_BUILD_TYPE=RELEASE \
      -DCMAKE_INSTALL_PREFIX=$PREFIX \
      -DCMAKE_C_COMPILER=occlum-gcc -DCMAKE_CXX_COMPILER=occlum-g++ \
      -DENABLE_PRECOMPILED_HEADERS=OFF \
      -DOPENCV_PC_FILE_NAME=opencv.pc \
      -DOPENCV_GENERATE_PKGCONFIG=ON \
      -DBUILD_opencv_java=OFF -DBUILD_JAVA_SUPPORT=OFF \
      -DBUILD_opencv_python=OFF -DBUILD_PYTHON_SUPPORT=OFF \
      -DBUILD_EXAMPLES=OFF -DWITH_FFMPEG=OFF \
      -DWITH_QT=OFF -DWITH_CUDA=OFF
    make -j
    sudo make install
    popd
}

build_tbb() {
    rm -rf deps/tbb_cmake && mkdir -p deps/tbb_cmake
    pushd deps/tbb_cmake
    git clone https://github.com/wjakob/tbb .
    git checkout 344fa84f34089681732a54f5def93a30a3056ab9
    mkdir cmake_build && cd cmake_build
    cmake ../ \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_CXX_COMPILER=occlum-g++ -DCMAKE_C_COMPILER=occlum-gcc \
      -DCMAKE_INSTALL_PREFIX=$PREFIX \
      -DTBB_BUILD_TESTS=OFF \
      -DTBB_BUILD_TBBMALLOC_PROXY=OFF
    make
    sudo make install
    popd
}

# Build OpenVINO
build_openvino() {
    rm -rf openvino_src && mkdir openvino_src
    pushd openvino_src
    git clone https://github.com/opencv/dldt .
    git checkout tags/2019_R3 -b 2019_R3
    git apply ../0001-Fix-passing-pre-increment-parameter-cpu-to-CPU_ISSET.patch
    cd inference-engine
    git submodule init
    git submodule update --recursive
    mkdir build && cd build
#   Substitute THREADING lib
    cmake ../ \
      -DTHREADING=$THREADING \
      -DENABLE_MKL_DNN=ON \
      -DENABLE_CLDNN=OFF \
      -DENABLE_MYRIAD=OFF \
      -DENABLE_GNA=OFF
    [ "$THREADING" == "OMP" ] && rm -rf ../temp/omp/lib/* && cp $PREFIX/lib/libgomp.so ../temp/omp/lib/libiomp5.so
    [ "$THREADING" == "TBB" ] && rm -rf ../temp/tbb/lib/* && cp $PREFIX/lib/libtbb.so ../temp/tbb/lib && cp $PREFIX/lib/libtbbmalloc.so ../temp/tbb/lib
    cd ../
    rm -rf build && mkdir build && cd build
#   Substitute OpenCV
    export OpenCV_DIR=$PREFIX/lib/cmake/opencv4
    cmake ../ \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_CXX_COMPILER=occlum-g++ -DCMAKE_CXX_FLAGS="-Wno-error=stringop-overflow=" \
      -DCMAKE_C_COMPILER=occlum-gcc -DCMAKE_C_FLAGS="-Wno-error=stringop-overflow=" \
      -DCMAKE_INSTALL_PREFIX=$PREFIX \
      -DTHREADING=$THREADING \
      -DENABLE_MKL_DNN=ON \
      -DENABLE_CLDNN=OFF \
      -DENABLE_OPENCV=OFF \
      -DENABLE_MYRIAD=OFF \
      -DENABLE_GNA=OFF
    make -j4
    popd
}

while [ -n "$1" ]; do
    case "$1" in
    --threading)    [ -n "$2" ] && THREADING=$2 ; shift 2 || show_usage ;;
    *)
        show_usage
    esac
done

if [ "$THREADING" == "TBB" ] ; then
    echo "Build OpenVINO with TBB threading"
    build_opencv
    build_tbb
    build_openvino
elif [ "$THREADING" == "OMP" ] ; then
    echo "Build OpenVINO with OpenMP threading"
    build_opencv
    build_openvino
else
    echo "Error: invalid threading: $THREADING"
    show_usage
fi
