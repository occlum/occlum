#!/usr/bin/env bash
set -e

export CC=occlum-gcc
export CXX=occlum-g++

THISDIR="$(dirname $(readlink -f $0))"
INSTALLDIR="/usr/local/occlum/x86_64-linux-musl"
DEPSDIR="$THISDIR/deps"
TARGET_SO="$DEPSDIR/openssl/libcrypto.so"
SGX_VER="2.11"

mkdir -p $DEPSDIR || exit 1

# Download OpenSSL 1.1.1
OPENSSLDIR="${DEPSDIR}/openssl"
if [ ! -d "$OPENSSLDIR" ] ; then
    echo "Downloading openssl ..."
    cd "$DEPSDIR" && \
    wget https://github.com/openssl/openssl/archive/OpenSSL_1_1_1.tar.gz && \
    tar -xvzf OpenSSL_1_1_1.tar.gz && \
    mv openssl-OpenSSL_1_1_1 openssl && \
    echo "Download openssl successfully" || exit 1
else
    echo "The openssl code is already existent"
fi

# Build openssl
if [ ! -f "$TARGET_SO" ] ; then
    echo "Building openssl ..."
    cd "$OPENSSLDIR" && \
    CC=occlum-gcc ./config \
      --prefix=$INSTALLDIR \
      --openssldir=/usr/local/occlum/ssl \
      --with-rand-seed=rdcpu \
      no-zlib no-async no-tests && \
    make -j${nproc} && make install && \
    echo "Build openssl successfully" || exit 1
else
    echo "The openssl library is aleady existent"
fi

# Download SGX SDK
SGX_SDK="${DEPSDIR}/linux-sgx-sdk"
if [ ! -d "$SGX_SDK" ] ; then
    echo "Downloading linux-sgx-sdk ..."
    cd "$DEPSDIR" && \
    wget https://github.com/intel/linux-sgx/archive/sgx_$SGX_VER.tar.gz && \
    tar -xvzf sgx_$SGX_VER.tar.gz && \
    mv linux-sgx-sgx_$SGX_VER linux-sgx-sdk && \
    echo "Download sgx-sdk successfully" || exit 1
else
    echo "The sgx-sdk code is already existent"
fi

# Copy files to DiffieHellmanLibrary dir
DH_DIR=$THISDIR/DiffieHellmanLibrary
DH_PATCH=DiffieHellmanLibrary.patch
mkdir -p $DH_DIR/include

cp $SGX_SDK/{sdk/ec_dh_lib/sgx_dh_internal.h,\
common/inc/internal/ecp_interface.h,\
common/inc/internal/ssl_wrapper.h\
} $DH_DIR/include

cp $SGX_SDK/{psw/ae/aesm_service/source/utils/crypto_aes_gcm.cpp,\
sdk/ec_dh_lib/ec_dh.cpp,\
sdk/tlibcrypto/sgxssl/sgx_cmac128.cpp,\
sdk/tlibcrypto/sgxssl/sgx_ecc256.cpp,\
sdk/tlibcrypto/sgxssl/sgx_sha256_msg.cpp\
} $DH_DIR

cd $DH_DIR && patch -p4 < $DH_PATCH >/dev/null 2>&1 || git apply $DH_PATCH -R --check
cd - >/dev/null 2>&1

echo "DiffieHellmanLibrary is ready"

# Copy header files from linux-sgx-sdk local attestation demo
cd $THISDIR
mkdir -p Include
cp $SGX_SDK/SampleCode/LocalAttestation/Include/{datatypes.h,\
dh_session_protocol.h,\
error_codes.h,\
fifo_def.h\
} Include

echo "Include is ready"
