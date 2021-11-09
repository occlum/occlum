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
    make -j4
    sudo make install
    popd
}

# Build oneTBB,  OpenVINO_2021 need version 2020.3
build_tbb() {
    rm -rf deps/tbb_cmake && mkdir -p deps/tbb_cmake
    pushd deps/tbb_cmake
    git clone https://github.com/oneapi-src/oneTBB.git .
    git checkout v2020.3
    CXX=occlum-g++ CC=occlum-gcc make tbb -j4
    find build/ -name libtbb* -exec cp {} $PREFIX/lib/ \;
    popd
}


# Build OpenVINO
build_openvino() {
    rm -rf openvino_src && mkdir openvino_src
    pushd openvino_src
    git clone https://github.com/opencv/dldt .
    git checkout 2021.3
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
    [ "$THREADING" == "OMP" ] && rm -rf ../inference-engine/temp/omp/lib/* && cp $PREFIX/lib/libgomp.so ../inference-engine/temp/omp/lib/libiomp5.so
    [ "$THREADING" == "TBB" ] && rm -rf ../inference-engine/temp/tbb/lib/* && cp $PREFIX/lib/libtbb.so.2 ../inference-engine/temp/tbb/lib
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
      -DENABLE_GNA=OFF \
      -DENABLE_VPU=OFF
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

# Tell CMake to search for packages in Occlum toolchain's directory only
export PKG_CONFIG_LIBDIR=$PREFIX/lib

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
