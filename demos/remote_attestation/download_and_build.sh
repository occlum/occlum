#!/usr/bin/env bash

export CC=occlum-gcc
export CXX=occlum-g++

THISDIR="$(dirname $(readlink -f $0))"
INSTALLDIR="/usr/local/occlum/x86_64-linux-musl"
DEPSDIR="$THISDIR/deps"

mkdir -p $DEPSDIR || exit 1

# Download OpenSSL 1.1.1
OPENSSLDIR="${DEPSDIR}/openssl"
if [ ! -d "$OPENSSLDIR" ] ; then
    echo "Downloading openssl ..."
    cd "$DEPSDIR" && \
    wget https://github.com/openssl/openssl/archive/OpenSSL_1_1_1.tar.gz && \
    tar -xvf OpenSSL_1_1_1.tar.gz && \
    mv openssl-OpenSSL_1_1_1 openssl && \
    echo "Download openssl successfully" || exit 1
else
    echo "The openssl code is already existent"
fi

# Download curl 7.29.0
CURLDIR="${DEPSDIR}/curl"
if [ ! -d "$CURLDIR" ] ; then
    echo "Downloading curl ..."
    cd "$DEPSDIR" && \
    wget  https://github.com/curl/curl/archive/curl-7_29_0.tar.gz && \
    tar -xvf curl-7_29_0.tar.gz && \
    mv curl-curl-7_29_0 curl && \
    echo "Download curl successfully" || exit 1
else
    echo "The openssl code is already existent"
fi

# Download other dependencies
CPPCODECDIR="${DEPSDIR}/cppcodec"
if [ ! -d "$CPPCODECDIR" ] ; then
    echo "Downloading cppcodec ..."
    cd "$DEPSDIR" && \
    wget -O cppcodec-v0.2.tar.gz \
        https://github.com/tplgy/cppcodec/archive/v0.2.tar.gz  && \
    tar -xvf cppcodec-v0.2.tar.gz && \
    mv cppcodec-0.2 cppcodec  && \
    echo "Download cppcodec successfully" || exit 1
else
    echo "The cppcodec code is already existent"
fi
RAPIDJSONDIR="${DEPSDIR}/rapidjson"
if [ ! -d "$RAPIDJSONDIR" ] ; then
    echo "Downloading rapidjson ..."
    cd "$DEPSDIR" && \
    wget -O rapidjson-v1.1.0.tar.gz \
        https://github.com/Tencent/rapidjson/archive/v1.1.0.tar.gz  && \
    tar -xvf rapidjson-v1.1.0.tar.gz && \
    mv rapidjson-1.1.0 rapidjson  && \
    echo "Download cppcodec successfully" || exit 1
else
    echo "The cppcodec code is already existent"
fi


# Build and install openssl
if [ ! -f "$INSTALLDIR/lib/libcrypto.so" ] ; then
    echo "Building openssl ..."
    cd "$OPENSSLDIR" && \
    CC=occlum-gcc ./config \
      --prefix=$INSTALLDIR \
      --openssldir=/usr/local/occlum/ssl \
      --with-rand-seed=rdcpu \
      no-zlib no-async no-tests && \
    make -j${nproc} && \
    sudo make install && \
    echo "Build openssl successfully" || exit 1
else
    echo "The openssl library is aleady existent"
fi

# Build and install libcurl
if [ ! -f "$INSTALLDIR/lib/libcurl.so" ] ; then
    cd "$CURLDIR"
    if [ ! -f ./configure ] ; then
      echo "Building configure file ..."
      ./buildconf || exit 1
    fi
    echo "Building curl ..."
    CC=occlum-gcc CXX=occlum-g++ ./configure \
      --prefix=$INSTALLDIR \
      --with-ssl=$INSTALLDIR && \
    make -j${nproc} && \
    sudo make install && \
    echo "Build curl successfully" || exit 1
else
    echo "The curl library is aleady existent"
fi

# Build the remote attestation library and demo application
echo "Build demo source code"
cd "$THISDIR" && rm -rf ./build && mkdir -p build
cd build && \
cmake -DCMAKE_CXX_COMPILER=occlum-g++ ../ && \
make -j$(nproc)
