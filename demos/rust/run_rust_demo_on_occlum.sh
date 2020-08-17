#!/bin/bash
set -e

# compile rust_app
pushd rust_app
occlum-cargo build
popd

# initialize occlum workspace
rm -rf occlum_instance && mkdir occlum_instance && cd occlum_instance

occlum init
cp ../rust_app/target/x86_64-unknown-linux-musl/debug/rust_app image/bin

occlum build
occlum run /bin/rust_app
