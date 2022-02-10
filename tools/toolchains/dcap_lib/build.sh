#!/bin/bash
set -e

INSTALL_DIR=/opt/occlum/toolchains/dcap_lib
SONAME=libocclum_dcap.so.0.1.0

function build_lib() {
    if [[ $1 == "musl" ]]; then
        echo "*** Build and install musl-libc dcap ***"
        CARGO=occlum-cargo
        TARGET_PATH=target/x86_64-unknown-linux-musl/release
        LIB_PATH=${INSTALL_DIR}/musl/
    else
        echo "*** Build and install glibc dcap ***"
        CARGO=cargo
        TARGET_PATH=target/release
        LIB_PATH=${INSTALL_DIR}/glibc/
    fi

    # cargo build libs and rust example
    $CARGO clean
    $CARGO rustc --release  -- -Clink-arg=-Wl,-soname,$SONAME
    $CARGO build --release  --examples

    # Copy files
    mkdir -p ${LIB_PATH}
    cp ${TARGET_PATH}/libocclum_dcap.a ${LIB_PATH}
    cp ${TARGET_PATH}/examples/dcap_test ${LIB_PATH}

    # Create SO links
    pushd ${TARGET_PATH}
    mv libocclum_dcap.so $SONAME
    ln -s $SONAME libocclum_dcap.so
    popd
    cp -Pf ${TARGET_PATH}/libocclum_dcap.so* ${LIB_PATH}
}

build_lib musl
build_lib glibc

cp -r inc ${INSTALL_DIR}/
