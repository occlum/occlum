#!/bin/bash
set -e

option=$1

rm -rf occlum-test
mkdir occlum-test && cd occlum-test
occlum init
mkdir -p image/usr/bin
cp ../Occlum.json .
cp ../fish-shell/build/fish image/usr/bin
cp ../busybox/busybox image/usr/bin
cp ../test_per_process_config.sh image/bin

# Set process memory space size to very small values and will fail when running target script using default configuration
new_json="$(jq '.process.default_stack_size = "1MB" |
                .process.default_heap_size = "1MB" |
                .process.default_mmap_size = "10MB"' Occlum.json)" && \
echo "${new_json}" > Occlum.json

pushd image/bin
ln -s /usr/bin/busybox cat
ln -s /usr/bin/busybox echo
ln -s /usr/bin/busybox awk
popd

# If `--without-ulimit` is specified, run without ulimit command and thus will fail
if [[ $1 == "--without-ulimit" ]]; then
    sed -i '/^ulimit -S/ s/^/# &/g' image/bin/test_per_process_config.sh
fi

occlum build
echo -e "\nBuild done. Running fish script ..."
occlum run /bin/test_per_process_config.sh
