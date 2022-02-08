#!/bin/bash
set -e

INSTALL_DIR=/opt/occlum/toolchains/dcap_lib

echo "*** Build and install musl-libc dcap ***"
occlum-cargo clean
occlum-cargo build --all-targets --release

mkdir -p ${INSTALL_DIR}/musl
cp target/x86_64-unknown-linux-musl/release/libocclum_dcap.a ${INSTALL_DIR}/musl/
cp target/x86_64-unknown-linux-musl/release/libocclum_dcap.so ${INSTALL_DIR}/musl/
cp target/x86_64-unknown-linux-musl/release/examples/dcap_test ${INSTALL_DIR}/musl/

echo "*** Build and install glibc dcap ***"
cargo clean
cargo build --all-targets --release

mkdir -p ${INSTALL_DIR}/glibc
cp target/release/libocclum_dcap.a ${INSTALL_DIR}/glibc/
cp target/release/libocclum_dcap.so ${INSTALL_DIR}/glibc/
cp target/release/examples/dcap_test ${INSTALL_DIR}/glibc/

cp -r inc ${INSTALL_DIR}/
