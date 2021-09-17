#!/bin/bash
set -e

# compile rust_app
pushd rust_app
occlum-cargo build
popd

# initialize occlum workspace
rm -rf occlum_instance && mkdir occlum_instance && cd occlum_instance

occlum init && rm -rf image
copy_bom -f ../rust-demo.yaml --root image --include-dir /opt/occlum/etc/template

occlum build
occlum run /bin/rust_app
